# Limites techniques de la signature des messages MAVLink 2

> **Portée** : ce document décrit uniquement MAVLink 2 et ses implémentations PX4 / ArduPilot.
> **Date de consultation de toutes les sources : 2026-10-07.** Les pages `mavlink.io` ne portent
> pas d'étiquette de version ; `docs.px4.io/main` correspond à la branche `main` de PX4 ;
> `ardupilot.org/dev` est le wiki développeur courant ; le code cité est celui de la branche
> `master` des dépôts (aucun SHA de commit n'a pu être capturé — voir *Gaps*).
> **Convention** : chaque constat est étiqueté **[SPEC]** (specification MAVLink), **[CODE]**
> (bibliothèque de référence `c_library_v2`), **[PX4]** ou **[ARDU]** (choix d'implémentation).

---

## Q1. Spécification officielle : fonctionnement exact de la signature (tailles, algorithme, couverture)

### Takeaway
La signature MAVLink 2 est un **bloc de 13 octets** ajouté en fin de trame : 1 octet d'identifiant
de lien, 6 octets d'horodatage, et **6 octets (48 bits) de SHA-256 tronqué** d'un HMAC-like
`sha256(clé_secrète ‖ en-tête ‖ charge utile ‖ CRC ‖ link-ID ‖ horodatage)`. C'est un **MAC
symétrique tronqué**, pas une signature asymétrique : il n'y a ni identité par nœud, ni
non-répudiation.

### Cited Findings
- **[SPEC]** Le bloc de signature fait 13 octets et occupe les octets `(n+12)` à `(n+24)` de la trame v2 ; la trame v2 signée maximale fait **280 octets** — [MAVLink Packet Serialization, mavlink.io](https://mavlink.io/en/guide/serialization.html)
- **[SPEC]** Le drapeau d'incompatibilité `0x01` est `MAVLINK_IFLAG_SIGNED` et indique qu'une trame est signée — [MAVLink Packet Serialization, mavlink.io](https://mavlink.io/en/guide/serialization.html)
- **[SPEC]** Link ID : « **8 bits** » ; « each implementation should assign a link ID to each of the MAVLink communication channels it has enabled » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[SPEC]** Horodatage : « **48 bits** » ; signature : « **48 bits (6 byte)** » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[SPEC]** Algorithme et couverture : `signature = sha256_48(secret_key + header + payload + CRC + link-ID + timestamp)`, avec un hachage **SHA-256** tronqué à 48 bits — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[SPEC]** Clé secrète : « **32 bytes of binary data** » ; elle « must not be exposed via any publicly accessible communication protocol » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[CODE]** Implémentation de référence en C — la troncature et l'ordre d'absorption sont explicites : le link ID est écrit en `signature[0]`, les 6 octets d'horodatage en `signature[1..6]`, puis `sha256_update` absorbe successivement `secret_key` (32 o), `header`, `packet`, `crc` (2 o) et les **7 premiers octets de la signature** (link ID + horodatage), avant `mavlink_sha256_final_48(&ctx, &signature[7])` :
  ```c
  signature[0] = signing->link_id;
  tstamp.t64 = signing->timestamp;
  memcpy(&signature[1], tstamp.t8, 6);
  signing->timestamp++;
  mavlink_sha256_init(&ctx);
  mavlink_sha256_update(&ctx, signing->secret_key, sizeof(signing->secret_key));
  mavlink_sha256_update(&ctx, header, header_len);
  mavlink_sha256_update(&ctx, packet, packet_len);
  mavlink_sha256_update(&ctx, crc, 2);
  mavlink_sha256_update(&ctx, signature, 7);
  mavlink_sha256_final_48(&ctx, &signature[7]);
  return MAVLINK_SIGNATURE_BLOCK_LEN;
  ```
  — [`mavlink_helpers.h`, mavlink/c_library_v2, branche master](https://raw.githubusercontent.com/mavlink/c_library_v2/master/mavlink_helpers.h)
- **[CODE]** La signature n'est produite que si le drapeau `MAVLINK_SIGNING_FLAG_SIGN_OUTGOING` est positionné ; sinon `mavlink_sign_packet()` retourne 0 (trame non signée) — [`mavlink_helpers.h`](https://raw.githubusercontent.com/mavlink/c_library_v2/master/mavlink_helpers.h)
- **[SPEC]** La structure d'état C contient deux pointeurs : `mavlink_signing_t *signing` (clé secrète, horodatage, drapeaux, par canal) et `mavlink_signing_streams_t *signing_streams` (horodatages précédents par tuple `(linkId, srcSystem, srcComponent)`), ce dernier devant être **commun à tous les canaux** — [C Message Signing (mavgen), mavlink.io](https://mavlink.io/en/mavgen_c/message_signing_c.html)
- **[SPEC]** `MAVLINK_MAX_SIGNING_STREAMS` « defaults to 16, but it may be worth raising this for GCS implementations » — [C Message Signing (mavgen), mavlink.io](https://mavlink.io/en/mavgen_c/message_signing_c.html)

### Inferences
- La construction `sha256(clé ‖ données)` est un **MAC par préfixe de clé**, pas un HMAC (RFC 2104). Pour SHA-256 (construction Merkle–Damgård), ce schéma est vulnérable aux **extensions de longueur** dans le cas général ; ici la surface est très réduite parce que la longueur du message est elle-même couverte (champ `len` de l'en-tête) et que la sortie est tronquée à 48 bits, mais le choix reste non conforme aux bonnes pratiques cryptographiques actuelles. *(inférence structurelle à partir du code cité ; aucune publication officielle ne qualifie ce point)*
- Troncature à 48 bits ⇒ probabilité de forge aveugle par tentative = **2⁻⁴⁸ ≈ 3,55 × 10⁻¹⁵** *(calcul)*. C'est faible, mais 48 bits est en dessous des 128 bits usuels pour un MAC ; sur un lien à haut débit avec oracle d'acceptation, la marge est bien moindre que celle d'un MAC complet.
- La couverture est **complète sur la partie immuable de la trame** : il n'existe pas de champ mutable par saut (pas d'équivalent TTL) dans MAVLink 2, donc tous les octets de la trame sauf les 6 octets de MAC sont authentifiés. *(inférence à partir de l'ordre d'absorption cité)*
- L'authentification est **symétrique** : tout détenteur de la clé peut forger n'importe quel couple `(sysid, compid)`. La signature prouve « un détenteur de la clé », **pas** « ce nœud précis ». Aucune non-répudiation. *(inférence directe de la formule de la spec)*

### Gaps
- La spec ne précise pas quelle implémentation de SHA-256 est normative ; la page mavgen C ne la documente pas non plus — [C Message Signing (mavgen)](https://mavlink.io/en/mavgen_c/message_signing_c.html).
- La page *Packet Serialization* mentionne les 13 octets mais **ne détaille pas** leur découpage interne (1 + 6 + 6) ; ce découpage n'est explicite que dans la page *Message Signing* et dans le code.
- Aucun SHA de commit n'a pu être relevé pour les fichiers de code cités (le client `gh` n'est pas disponible dans l'environnement) ; seule la branche `master` au 2026-10-07 est garantie.

---

## Q2. Unité, origine de l'horodatage et fenêtre de rejeu acceptée

### Takeaway
L'horodatage est sur **48 bits en unités de 10 µs depuis le 1ᵉʳ janvier 2015 GMT**. La règle
principale est la **monotonie stricte par flux logique** `(SystemID, ComponentID, LinkID)` ; la
fenêtre d'« une minute » (**6 000 000 unités = 60 s**) ne s'applique qu'à l'**acceptation d'un
flux inconnu**, pas aux flux déjà connus.

### Cited Findings
- **[SPEC]** « 10 microsecond units since 1st January 2015 GMT time » ; conversion vers l'heure Unix par « an offset in seconds of **1420070400** » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[SPEC]** « This *must* monotonically increase for every message on a particular link » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[SPEC]** Le flux logique est le tuple **`(SystemID, ComponentID, LinkID)`** ; l'exigence de monotonie s'applique séparément à chaque combinaison — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[SPEC]** Règles de traitement de l'horodatage, telles qu'énoncées :
  - stocker l'horodatage régulièrement en mémoire persistante, « ideally at least once a minute » ;
  - au démarrage, prendre le **maximum** entre l'horloge système et l'horodatage stocké ;
  - incrémenter l'horodatage de 1 pour chaque message émis sur un lien donné ;
  - ne mettre à jour l'horodatage entrant **que s'il est supérieur** à l'horodatage courant ;
  - **ne jamais** mettre à jour l'horodatage d'un lien depuis un paquet mal signé ;
  - rejeter tout horodatage entrant inférieur au précédent du même flux ;
  - accepter un **nouveau** flux si son horodatage n'est pas en retard de plus de **6 millions d'unités (une minute)** sur l'horodatage local
  — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[CODE]** La bibliothèque C « automatically increments timestamps by one per outgoing message, updates incoming timestamps when greater than current local values, and rejects messages with older timestamps » ; « It is the responsibility of each MAVLink system to store and restore the timestamp into persistent storage » — [C Message Signing (mavgen), mavlink.io](https://mavlink.io/en/mavgen_c/message_signing_c.html)
- **[CODE]** La vérification d'horodatage de `mavlink_signature_check()` implémente la fenêtre d'une minute pour les flux nouveaux, sous la forme d'une comparaison `horodatage_reçu + 60 s < signing->timestamp` — [`mavlink_helpers.h`, c_library_v2](https://raw.githubusercontent.com/mavlink/c_library_v2/master/mavlink_helpers.h) ⚠️ *l'orthographe exacte de la constante n'a pas pu être confirmée mot pour mot (voir Gaps) ; la valeur de 60 s est, elle, confirmée par la spec.*

### Inferences
- **La « fenêtre de rejeu » n'est pas une fenêtre glissante d'anti-rejeu au sens d'IPsec.** Il n'y a pas de fenêtre de bitmap tolérant le désordre : pour un flux connu, tout paquet dont l'horodatage est ≤ au dernier accepté est rejeté. Conséquence directe : **MAVLink signing ne tolère pas le réordonnancement** sur un transport non ordonné (UDP, radio) — un paquet arrivé en retard est indistinguable d'un rejeu et est détruit. *(inférence à partir des règles citées)*
- Corollaire opérationnel : un nœud qui **redémarre** et repart d'un horodatage inférieur est **verrouillé** par le récepteur, dont la table de flux contient encore une valeur haute, et la fenêtre de 60 s ne l'aide pas (elle ne s'applique qu'aux flux *inconnus*). D'où les contournements spécifiques à ArduPilot (avance de +1 min au démarrage, cf. Q6). *(inférence)*
- Plage de l'horodatage 48 bits : 2⁴⁸ × 10 µs ≈ 2,815 × 10⁹ s ≈ **89 ans**, soit un débordement vers **2104** *(calcul)*. Non limitant en pratique.
- `MAVLINK_MAX_SIGNING_STREAMS = 16` par défaut implique qu'une station sol suivant plus de 16 tuples `(linkid, sysid, compid)` distincts épuise la table — la doc mavgen recommande explicitement de l'augmenter pour les GCS, ce qui est une **limite d'échelle de flotte** et non de sécurité. *(inférence appuyée par la recommandation de la doc)*

### Gaps
- Le nom exact de la constante encodant la fenêtre de 60 s dans `mavlink_helpers.h` n'a pas pu être vérifié textuellement (le rendu obtenu est une paraphrase possible). **Ne pas citer de nom de constante sans relecture directe du fichier.**
- La spec ne dit pas ce qu'il faut faire quand la table de flux est pleine ; seule la valeur par défaut de 16 est documentée.

---

## Q3. Clé partagée par lien ou propre à chaque nœud ? Combien de clés pour une flotte ?

### Takeaway
La spec ne définit **qu'une clé symétrique de 32 octets par lien** (« only one key per link » dans
les bibliothèques standard). Il n'y a **aucune notion d'identité cryptographique par nœud** :
MAVLink signing est un secret partagé, donc un unique secret pour un domaine de confiance, et tout
nœud compromis peut usurper n'importe quel autre nœud du même domaine.

### Cited Findings
- **[SPEC]** « only one key per link » dans les bibliothèques standard ; clé de « 32 bytes of binary data » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[SPEC]** La clé est transmise par le message `SETUP_SIGNING` sur un « secure link (e.g. USB or wired Ethernet) » ; « The `SETUP_SIGNING` message should never be broadcast, and received `SETUP_SIGNING` messages must never be automatically forwarded » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[CODE]** `signing.link_id = (uint8_t)chan;` — l'identifiant de lien est dérivé du numéro de canal, la clé étant copiée dans la structure `signing` propre à ce canal : `memcpy(signing.secret_key, key.secret_key, 32);` — [C Message Signing (mavgen), mavlink.io](https://mavlink.io/en/mavgen_c/message_signing_c.html)
- **[ARDU]** La clé est stockée dans une structure `SigningKey { uint32_t magic = 0x3852fcd1; uint64_t timestamp; uint8_t secret_key[32]; }` en **FRAM/`hal.storage`** ; lors du chargement, la clé 32 octets est copiée et le link ID est positionné, puis la clé est « activate[d] immediately on all links » — [`GCS_Signing.cpp`, ArduPilot master](https://raw.githubusercontent.com/ArduPilot/ardupilot/master/libraries/GCS_MAVLink/GCS_Signing.cpp)
- **[PX4]** La clé et l'horodatage sont dans un fichier **unique** de 40 octets sur la carte SD : `/mavlink/mavlink-signing-key.bin` (octets 0–31 = clé, 32–39 = horodatage `uint64_t` little-endian), permissions `0600`, répertoire `0700` — [MAVLink Message Signing, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/message_signing.html) ; confirmé par la source markdown — [`docs/en/mavlink/message_signing.md`, PX4-Autopilot main](https://raw.githubusercontent.com/PX4/PX4-Autopilot/main/docs/en/mavlink/message_signing.md)

### Inferences
- **Arithmétique de flotte** *(inférence/estimation)* :
  - *Clé unique de flotte* : **1 clé** à gérer, mais la compromission d'un seul véhicule (ou d'une seule radio, ou d'une seule carte SD) donne le pouvoir de forger pour **toute** la flotte. Aucune résistance à l'attaquant interne.
  - *Clé par véhicule* : **N clés** ; chaque station sol autorisée doit détenir les N clés, et chaque relais sur le chemin doit détenir la clé du segment qu'il signe — le nombre de copies du secret croît en N × (nombre de GCS), ce qui augmente la surface d'exfiltration plutôt que de la réduire.
  - *Par lien/canal* : la spec plafonnant à une clé par lien, un nœud à k liens ne peut pas appliquer k politiques de clés différentes au-delà de k structures `signing` distinctes, et la table `signing_streams` doit rester commune.
- Il n'existe **aucun mécanisme d'attestation, d'enrôlement ou de liaison clé↔identité de nœud** dans la spec : `SETUP_SIGNING` installe un secret, il n'y a pas de preuve de possession ni de certificat. *(inférence : absence constatée dans la page de spec)*
- PX4 concentre tout le matériel cryptographique dans **un fichier sur un support amovible** ; ArduPilot le place en FRAM interne (non amovible). Du point de vue de l'extraction physique, le choix ArduPilot est strictement plus difficile à exploiter, les deux restant sans élément sécurisé. *(inférence comparative appuyée sur les deux sources ci-dessus)*

### Gaps
- Aucune source officielle ne donne de recommandation chiffrée sur la **granularité des clés en flotte** (une par flotte vs une par véhicule). L'arithmétique ci-dessus est une déduction, pas une position projet.

---

## Q4. La signature est-elle activée par défaut dans PX4 et dans ArduPilot ?

### Takeaway
**Non dans les deux cas.** PX4 le dit explicitement : « Signing is **disabled by default** » et « By
default, all MAVLink messages are unauthenticated and unencrypted ». ArduPilot n'active la
signature que si une clé a été provisionnée (sinon le chargement échoue et la signature est
désactivée), et son wiki précise en plus que le port série doit être réglé sur MAVLink2 pour que la
protection s'applique.

### Cited Findings
- **[PX4]** « Signing is **disabled by default** » ; « No key on SD card: Signing is disabled. All messages are sent unsigned and all incoming messages are accepted » — [MAVLink Message Signing, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/message_signing.html)
- **[PX4]** « **By default, all MAVLink messages are unauthenticated and unencrypted.** » et « production deployments must secure the link » — [MAVLink Security Hardening for Production Deployments, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/security_hardening.html)
- **[PX4]** Sans sécurisation, « any device that can send MAVLink messages to the vehicle (via radio, network, or serial) » peut exécuter des commandes : accès shell, opérations fichiers, modification de paramètres, upload de mission, armement, terminaison de vol — [MAVLink Security Hardening, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/security_hardening.html)
- **[PX4]** La configuration passe par le message `SETUP_SIGNING` (ID 256) et **non par un paramètre** : « The secret key is stored in a separate file on the SD card, not as a MAVLink parameter, so it cannot be read back through the parameter protocol » — [MAVLink Message Signing, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/message_signing.html)
- **[PX4]** Les changements de clé (activation, désactivation, rotation) sont « **rejected while the vehicle is armed** » — [MAVLink Message Signing, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/message_signing.html)
- **[ARDU]** `load_signing_key()` : si la signature est désactivée par option ou si le chargement de la clé échoue, la fonction sort immédiatement ; la signature est désactivée si l'horodatage **et** les octets de clé sont nuls — [`GCS_Signing.cpp`, ArduPilot master](https://raw.githubusercontent.com/ArduPilot/ardupilot/master/libraries/GCS_MAVLink/GCS_Signing.cpp)
- **[ARDU]** `handle_setup_signing()` refuse la configuration **quand le véhicule est armé**, décode le message, crée une nouvelle `SigningKey` avec l'horodatage initial fourni, l'enregistre, puis « activate[s] it immediately on all links » — [`GCS_Signing.cpp`, ArduPilot master](https://raw.githubusercontent.com/ArduPilot/ardupilot/master/libraries/GCS_MAVLink/GCS_Signing.cpp)
- **[ARDU]** Mise en place via Mission Planner : menu SETUP → sous-menu Advanced → « Mavlink Signing » ; bouton ADD pour créer une passkey, bouton USE pour l'activer sur l'autopilote connecté, bouton « Disable Signing » pour la retirer — [MAVLink2 Signing, ardupilot.org/dev](https://ardupilot.org/dev/docs/common-MAVLink2-signing.html)
- **[ARDU]** Le protocole du port de télémétrie doit être réglé sur « option = 2 (MAVLink2) » pour que la protection fonctionne — [MAVLink2 Signing, ardupilot.org/dev](https://ardupilot.org/dev/docs/common-MAVLink2-signing.html)

### Inferences
- Les deux projets convergent sur un modèle **« opt-in par provisionnement »** : il n'y a pas d'interrupteur booléen, c'est la **présence d'une clé** qui active la fonction. Une flotte non provisionnée est donc entièrement non authentifiée, silencieusement. *(inférence appuyée par les deux docs)*
- Le refus de changer de clé en vol (PX4 et ArduPilot) est un garde-fou de sûreté, mais il signifie aussi qu'**aucune révocation d'urgence n'est possible pendant un vol**. *(inférence)*

### Gaps
- La page wiki ArduPilot citée **ne déclare pas explicitement** « disabled by default » ; la conclusion repose sur le code (`load_signing_key`) et sur l'ergonomie décrite (il faut créer puis « USE » une passkey). À confirmer par une lecture directe du fichier source.
- Un enregistrement CVE apparu dans les résultats de recherche (**CVE-2026-1579**, [db.gcve.eu](https://db.gcve.eu/vuln/cve-2026-1579)) porterait précisément sur l'absence d'authentification MAVLink par défaut dans PX4 et l'accès shell via `SERIAL_CONTROL`. ⚠️ **Non vérifié** : seule une bribe de résultat de recherche a été vue, la fiche complète n'a pas été récupérée. À confirmer avant toute citation.

---

## Q5. Déclassement vers MAVLink 1 (non signé) : possible ? Comment s'en protège-t-on ?

### Takeaway
MAVLink 1 (`0xFE`) n'a **aucun champ de signature** : la signature est une fonctionnalité v2
signalée par un drapeau d'incompatibilité. La spec ne prévoit **aucune négociation authentifiée de
version** ; la seule protection est la **politique locale d'acceptation des paquets non signés**, et
l'une des politiques que la spec suggère elle-même (« accepter jusqu'à réception du premier paquet
signé ») est intrinsèquement vulnérable au déclassement.

### Cited Findings
- **[SPEC]** La version est déterminée par le premier octet de la trame : MAVLink 1 = `0xFE`, MAVLink 2 = `0xFD` — [MAVLink Versions, mavlink.io](https://mavlink.io/en/guide/mavlink_version.html)
- **[SPEC]** Limite explicite : « **A system cannot use a single channel to connect to signed MAVLink 2 systems, unsigned MAVLink 2 systems, and/or MAVLink 1 components.** » — [MAVLink Versions, mavlink.io](https://mavlink.io/en/guide/mavlink_version.html)
- **[SPEC]** Comportement de démarrage recommandé : **si la signature est désactivée** et que MAVLink 2 est activé, un véhicule « may choose to start by sending MAVLink 1 and switch to MAVLink 2 on a link when it first receives a MAVLink 2 message on the link » — [MAVLink Versions, mavlink.io](https://mavlink.io/en/guide/mavlink_version.html)
- **[SPEC]** La capacité v2 est annoncée par le drapeau `MAV_PROTOCOL_CAPABILITY_MAVLINK2` dans `AUTOPILOT_VERSION`, y compris « in the case where the link is currently sending MAVLink 1 packets but MAVLink 2 packets will be accepted » — [MAVLink Versions, mavlink.io](https://mavlink.io/en/guide/mavlink_version.html)
- **[SPEC]** Politiques d'acceptation des paquets non signés suggérées, *verbatim* :
  - « Accept all unsigned packets based on a system-specific parameter. »
  - « Accept all unsigned packets if the connection is over a "secure channel" »
  - « `RADIO_STATUS` packets are always accepted without signing »
  - « Accept all unsigned packets when in an "unsigned mode" »
  - « **Accept all unsigned packets until a signed packet is received** »
  — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[ARDU]** La protection ne s'applique qu'« via a SERIAL port using MAVLink2 protocol » ; le port doit être configuré « option = 2 (MAVLink2) » — [MAVLink2 Signing, ardupilot.org/dev](https://ardupilot.org/dev/docs/common-MAVLink2-signing.html)
- **[ARDU]** Les connexions **USB restent non affectées**, « allowing full connection to an autopilot using an unknown passkey » — [MAVLink2 Signing, ardupilot.org/dev](https://ardupilot.org/dev/docs/common-MAVLink2-signing.html)
- **[ARDU]** `accept_unsigned_callback()` accepte les paquets non signés : **toujours sur `MAVLINK_COMM_0`** (« assumed to be secure channel », l'USB sur toutes les cartes), et, quel que soit le canal, pour les messages `MAVLINK_MSG_ID_RADIO_STATUS` et `MAVLINK_MSG_ID_RADIO` — [`GCS_Signing.cpp`, ArduPilot master](https://raw.githubusercontent.com/ArduPilot/ardupilot/master/libraries/GCS_MAVLink/GCS_Signing.cpp)
- **[PX4]** `SETUP_SIGNING` n'est accepté **que sur une connexion USB** ; le message est « silently ignored on all other link types (telemetry radios, network, and so on) » — [MAVLink Message Signing, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/message_signing.html). ⚠️ Une seconde lecture de la **source markdown** du même document donne une formulation différente : « PX4 accepts this on any link, but **once signing is active, key changes must arrive as signed messages using the current key** » — [`message_signing.md`, PX4-Autopilot main](https://raw.githubusercontent.com/PX4/PX4-Autopilot/main/docs/en/mavlink/message_signing.md). Les deux lectures **se contredisent** sur la restriction USB pour le premier provisionnement : à départager par lecture du code PX4 (`mavlink_receiver.cpp`).

### Inferences
- **Le déclassement est possible au niveau du cadrage** : rien dans la trame MAVLink 1 ne peut être authentifié, donc un attaquant qui émet du `0xFE` contourne entièrement la vérification de signature *si et seulement si* la politique locale accepte les paquets non signés sur ce canal. La protection n'est pas protocolaire, elle est **politique**. *(inférence appuyée sur la citation de la spec et sur `accept_unsigned_callback`)*
- **Chemin de déclassement concret chez ArduPilot** : un accès physique au port USB donne un canal non signé à pleins pouvoirs (confirmé par le wiki *et* par le code, qui accepte inconditionnellement `MAVLINK_COMM_0`). La signature protège donc le lien radio, pas la machine.
- **Chemin de déclassement par configuration** : laisser `SERIALx_PROTOCOL` sur MAVLink 1 neutralise la signature sans alerte. *(inférence à partir du wiki ArduPilot)*
- La politique « accepter jusqu'au premier paquet signé » que la spec propose crée une **fenêtre de démarrage** exploitable : qui parle en premier impose le mode. *(inférence)*
- La phrase de la spec « si la signature est désactivée… commencer en MAVLink 1 » montre que le projet a bien perçu l'incompatibilité : **signature et rétrocompatibilité v1 sur un même canal sont mutuellement exclusives**.

### Gaps
- **Contradiction non résolue** (ci-dessus) sur la restriction USB du `SETUP_SIGNING` dans PX4. Les deux citations proviennent du *même* document officiel via deux rendus différents ; une relecture directe du code PX4 est nécessaire.
- Aucun document officiel trouvé qui traite explicitement du **« downgrade attack »** comme classe de menace nommée, ni dans la spec, ni dans PX4 *security hardening* (ce dernier « does not address downgrade risks »).

---

## Q6. Horodatage de signature et heure GPS : une usurpation GPS permet-elle un rejeu ? (ArduPilot #13860)

### Takeaway
Oui, le lien existe et il est documenté : la spec elle-même fonde l'horodatage sur une horloge
absolue et ArduPilot le met à jour depuis l'heure GPS UTC. Le ticket **ArduPilot #13860** (2020)
établit qu'une usurpation GPS déplace l'horodatage de signature ; les mainteneurs ont répondu que
la monotonie forcée en session et la persistance en FRAM limitent l'attaque à un scénario
nécessitant un **effacement préalable du stockage**. Le ticket a été **fermé en « completed » en
4 jours, avec pour seule issue une mention en documentation** — aucun changement de protocole.

### Cited Findings
- **[SPEC]** L'horodatage est une horloge absolue (10 µs depuis 2015-01-01 GMT) et au démarrage il faut prendre « the maximum of system clock and stored timestamp values » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[ARDU]** `update_signing_timestamp()` reçoit une valeur **UTC en microsecondes**, soustrait le décalage d'époque (1970 → 2015), convertit en unités de 10 µs et met à jour tous les canaux où la signature est active — [`GCS_Signing.cpp`, ArduPilot master](https://raw.githubusercontent.com/ArduPilot/ardupilot/master/libraries/GCS_MAVLink/GCS_Signing.cpp)
- **[ARDU]** `save_signing_timestamp()` persiste « every 30s, unless **forced by a GPS update** », et n'écrit que si l'horodatage courant dépasse la valeur stockée — [`GCS_Signing.cpp`, ArduPilot master](https://raw.githubusercontent.com/ArduPilot/ardupilot/master/libraries/GCS_MAVLink/GCS_Signing.cpp)
- **[ARDU]** Au chargement, l'horodatage est avancé de « 1 minute past the last recorded » pour contrer le rejeu — [`GCS_Signing.cpp`, ArduPilot master](https://raw.githubusercontent.com/ArduPilot/ardupilot/master/libraries/GCS_MAVLink/GCS_Signing.cpp)
- **[ARDU #13860]** Titre : « MAVLink 2.0 message signing can be broken under GPS spoofing », auteur **KimHyungSub**, ouvert le **2020-03-22T18:43:23Z**, fermé le **2020-03-26T03:59:42Z**, état **closed / completed** — [issue #13860, API GitHub](https://api.github.com/repos/ArduPilot/ardupilot/issues/13860) · [page de l'issue](https://github.com/ArduPilot/ardupilot/issues/13860)
- **[ARDU #13860]** Corps, *verbatim* : « But, when I conducted GPS spoofing, ArduPilot updated both system clock and timestamp for the message signing. […] It means that anyone controls the timestamp and then conducts some replay attacks without the secret key. » Solution proposée : « One possible solution is that ArduPilot does not update the timestamp after initiating the message signing even though ArduPilot gets GPS lock. » Versions : 3DR IRIS+, Pixhawk1, Copter 3.5.5, « All » plateformes/airframes — [issue #13860](https://github.com/ArduPilot/ardupilot/issues/13860)
- **[ARDU #13860]** Réponse de **tridge** (2020-03-22T21:42) : « This doesn't really make sense. **We force the 64 bit timestamp to only ever go forward.** » — [commentaires de l'issue](https://api.github.com/repos/ArduPilot/ardupilot/issues/13860/comments)
- **[ARDU #13860]** Réponse de **peterbarker** (2020-03-22T21:37) : « We do have protection for the timestamp going backwards during a session… We also persist the signing timestamp in storage for future boots… » — [commentaires](https://api.github.com/repos/ArduPilot/ardupilot/issues/13860/comments)
- **[ARDU #13860]** Contre-argument du rapporteur (2020-03-22T22:03) : ArduPilot démarre avec un horodatage au **1ᵉʳ janvier 1970** avant détection GPS, ce qui permet à un attaquant de fixer l'horodatage via un signal GPS usurpé correspondant à une date ancienne, puis de rejouer des messages capturés antérieurement — [commentaires](https://api.github.com/repos/ArduPilot/ardupilot/issues/13860/comments)
- **[ARDU #13860]** **tridge** (2020-03-23T09:04) : « The only scenario would be if the user deliberately reset all of storage… It is stored in `hal.storage`, **which is not on the microSD card.** » — [commentaires](https://api.github.com/repos/ArduPilot/ardupilot/issues/13860/comments)
- **[ARDU #13860]** Le rapporteur confirme (2020-03-25T23:56) qu'un **effacement du stockage est requis**, et note que les récepteurs u-blox font confiance aux satellites usurpés dont le signal est plus fort que le légitime — [commentaires](https://api.github.com/repos/ArduPilot/ardupilot/issues/13860/comments)
- **[ARDU #13860]** Clôture par **peterbarker** (2020-03-26T03:59) : il indique qu'un avertissement existe déjà dans la documentation, corrige une affirmation antérieure sur un déni de service, et remercie le contributeur pour la revue de sécurité — [commentaires](https://api.github.com/repos/ArduPilot/ardupilot/issues/13860/comments)
- **[PX4 — contraste]** L'horodatage sur disque est mis à jour « when `SETUP_SIGNING` is received » et lors d'un arrêt propre ; mais « since most vehicles are powered off by pulling the battery, the on-disk timestamp will typically remain at the value from the last key provisioning on reboot » — [MAVLink Message Signing, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/message_signing.html)

### Inferences
- **Deux choix d'implémentation opposés, deux faiblesses différentes** *(inférence)* :
  - ArduPilot **lie** l'horodatage de signature à l'heure GPS ⇒ exposition à l'usurpation GNSS, atténuée par monotonie en session + persistance FRAM + avance de +1 min au boot. Le vecteur résiduel est : *effacer le stockage*, puis usurper le GPS avant la première acquisition légitime.
  - PX4 **ne lie pas** l'horodatage au GPS (écriture seulement sur `SETUP_SIGNING` et arrêt propre) ⇒ immunité structurelle au vecteur #13860, mais l'horodatage sur disque reste **figé à la date de provisionnement**, donc après chaque coupure brutale le compteur local repart très bas, et la fenêtre de 60 s pour les flux inconnus s'applique relativement à cette valeur basse.
- **Fenêtre de régression ArduPilot** : persistance toutes les 30 s ⇒ après une coupure brutale, jusqu'à ~30 s de progression d'horodatage sont perdus (mitigés par l'avance de +1 min au chargement). *(calcul à partir du code cité)*
- La position des mainteneurs est **correcte mais conditionnelle** : la monotonie forcée protège *dans une session* et *si le stockage persistant est intact*. Elle ne constitue pas une défense contre un attaquant ayant eu un accès physique (ce que la signature n'a de toute façon jamais prétendu couvrir).
- Le ticket s'est conclu **sans modification du protocole ni de l'implémentation** : la réponse officielle est documentaire. C'est un point citable important sur la maturité du modèle de menace. *(inférence à partir du fil et de l'état « completed »)*

### Gaps
- **Aucun ticket PX4 équivalent n'a été trouvé.** Les recherches ne remontent que des fils de forum non officiels ([discuss.px4.io/t/mavlink-security-problem/5921](https://discuss.px4.io/t/mavlink-security-problem/5921), [discuss.px4.io/t/px4-mavlink2-signing/49306](https://discuss.px4.io/t/px4-mavlink2-signing/49306)) — **forum ≠ source primaire**, à ne pas utiliser seul.
- Le ticket #13860 ne référence **aucune PR ni commit** de correction dans les données récupérées ; le texte exact de l'« avertissement déjà dans la documentation » mentionné par peterbarker n'a pas été localisé.
- La vidéo de démonstration citée dans le ticket (`youtu.be/JsUApnQAe7o`) n'a pas été consultée.

---

## Q7. Que devient un message non signé quand la signature est activée ? Quelles exceptions ?

### Takeaway
La spec **n'impose pas** le rejet : elle demande seulement aux bibliothèques de fournir un mécanisme
d'acceptation conditionnelle (`accept_unsigned_callback`), et suggère `RADIO_STATUS` comme exception.
Les deux implémentations ont étendu cette liste différemment : **PX4 exempte 4 messages
(HEARTBEAT, RADIO_STATUS, ADSB_VEHICLE, COLLISION)**, **ArduPilot exempte 2 messages (RADIO,
RADIO_STATUS) plus la totalité du canal 0 (USB)**.

### Cited Findings
- **[SPEC]** « MAVLink libraries should provide a mechanism that allows a system to **conditionally accept** *unsigned* packets » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[SPEC]** Les 5 politiques suggérées (liste *verbatim* reproduite en Q5), dont « `RADIO_STATUS` packets are always accepted without signing » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[SPEC/CODE]** Le point d'extension est le pointeur de fonction `accept_unsigned_callback` installé dans `mavlink_signing_t` — [C Message Signing (mavgen), mavlink.io](https://mavlink.io/en/mavgen_c/message_signing_c.html)
- **[PX4]** Quatre messages **toujours acceptés non signés**, quel que soit l'état de la signature : `HEARTBEAT` (ID **0**, découverte système), `RADIO_STATUS` (ID **109**, état du lien radio), `ADSB_VEHICLE` (ID **246**, trafic pour l'évitement de collision), `COLLISION` (ID **247**, alertes de collision) — [MAVLink Message Signing, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/message_signing.html) ; **liste confirmée à l'identique** par la source markdown — [`message_signing.md`, PX4-Autopilot main](https://raw.githubusercontent.com/PX4/PX4-Autopilot/main/docs/en/mavlink/message_signing.md)
- **[PX4]** Avertissement explicite : « **An attacker can spoof these specific messages (e.g. fake `ADSB_VEHICLE` traffic) even when signing is active.** » — [MAVLink Message Signing, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/message_signing.html)
- **[PX4]** « Unsigned or incorrectly signed messages are **not forwarded to other links** either. » — [`message_signing.md`, PX4-Autopilot main](https://raw.githubusercontent.com/PX4/PX4-Autopilot/main/docs/en/mavlink/message_signing.md)
- **[PX4]** Un « small set of safety-critical messages » dont `HEARTBEAT` et `RADIO_STATUS` sont « **always accepted unsigned on all links** » — [MAVLink Security Hardening, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/security_hardening.html)
- **[ARDU]** `accept_unsigned_callback()` : accepte **systématiquement** `MAVLINK_COMM_0` (canal supposé sûr, l'USB sur toutes les cartes) ; sinon n'accepte que `MAVLINK_MSG_ID_RADIO_STATUS` et `MAVLINK_MSG_ID_RADIO` — [`GCS_Signing.cpp`, ArduPilot master](https://raw.githubusercontent.com/ArduPilot/ardupilot/master/libraries/GCS_MAVLink/GCS_Signing.cpp)
- **[ARDU]** Côté effets : une fois la signature active, « any link via a SERIAL port using MAVLink2 protocol will **only respond to MAVLink commands** if they are signed with the key the autopilot is using » — mais « **links will still receive telemetry updates even if they are not using signing with the active key** » — [MAVLink2 Signing, ardupilot.org/dev](https://ardupilot.org/dev/docs/common-MAVLink2-signing.html)
- **[ARDU]** « This does **NOT** encrypt the data, just merely controls if the autopilot will respond to MAVLink commands » — [MAVLink2 Signing, ardupilot.org/dev](https://ardupilot.org/dev/docs/common-MAVLink2-signing.html)
- **[CODE]** La spec impose de **ne jamais** mettre à jour l'horodatage d'un lien à partir d'un paquet mal signé — ce qui ferme le vecteur « empoisonner le compteur avec une trame invalide » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)

### Inferences
- **La liste d'exemptions est le périmètre réel de la sécurité.** Chaque message exempté est un canal d'injection non authentifié. Le cas le plus lourd est PX4 : `ADSB_VEHICLE` et `COLLISION` alimentent la logique d'évitement, donc une injection non signée peut influencer le comportement du véhicule — PX4 le reconnaît explicitement. *(inférence appuyée sur l'avertissement cité)*
- **Divergence spec/implémentations** : la spec ne suggère **qu'un** message (`RADIO_STATUS`) ; PX4 en exempte 4 et ArduPilot 2 + un canal entier. Il n'y a donc **pas d'interopérabilité du modèle de menace** entre deux autopilotes « conformes ». *(inférence)*
- ArduPilot sépare **réception de télémétrie** (non protégée, toujours émise) de **exécution de commandes** (protégée). La signature chez ArduPilot est donc un **contrôle d'accès à l'actionnement**, pas un contrôle d'accès au lien. *(inférence appuyée sur le wiki)*
- `HEARTBEAT` non signé chez PX4 permet à un attaquant de se faire découvrir comme système/composant arbitraire, ce qui peut suffire à provoquer des bascules de mode de type « perte de liaison GCS »/failsafe selon la configuration. *(inférence — non confirmée par une source officielle, à vérifier)*

### Gaps
- Le code source PX4 correspondant à la liste des 4 exemptions (probablement dans `mavlink_receiver.cpp` / le rappel `accept_unsigned`) **n'a pas été lu directement** ; seule la documentation officielle est citée.
- Je n'ai pas trouvé de source officielle confirmant qu'un `HEARTBEAT` usurpé non signé peut déclencher un failsafe (inférence Q7 ci-dessus).

---

## Q8. Rotation, révocation, appairage : mécanismes documentés ?

### Takeaway
**Il n'existe ni révocation, ni rotation automatique, ni appairage cryptographique.** La seule
« cérémonie » est l'envoi de `SETUP_SIGNING` sur un canal supposé sûr (USB / Ethernet filaire), et
la seule « révocation » est la ré-provision manuelle ou la suppression de la clé, véhicule par
véhicule et désarmé. PX4 l'écrit noir sur blanc.

### Cited Findings
- **[PX4]** « There is **no automatic key rotation**. Keys must be reprovisioned manually via a signed `SETUP_SIGNING` message. » — [MAVLink Security Hardening, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/security_hardening.html)
- **[PX4]** Récupération après perte de clé : « the only recovery is **physical**: remove the SD card and delete the key file, or reflash via SWD/JTAG » — [MAVLink Security Hardening, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/security_hardening.html)
- **[PX4]** La récupération d'une clé perdue requiert « physical access to the SD card or debug port » ; parmi les limites listées figure l'absence de « automatic key rotation mechanism » — [MAVLink Message Signing, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/message_signing.html)
- **[PX4]** Toute modification (activation, désactivation, **rotation**) est refusée véhicule armé — [MAVLink Message Signing, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/message_signing.html)
- **[SPEC]** Le seul mécanisme de distribution est `SETUP_SIGNING` sur un « secure link (e.g. USB or wired Ethernet) », avec interdiction de diffusion et de réacheminement automatique — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)
- **[ARDU]** Gestion de clés côté station sol : liste des passkeys dans Mission Planner, bouton ADD pour créer, sélection + suppression puis sauvegarde pour retirer, bouton USE pour activer sur l'autopilote connecté, « Disable Signing button » pour désactiver sur l'autopilote connecté — [MAVLink2 Signing, ardupilot.org/dev](https://ardupilot.org/dev/docs/common-MAVLink2-signing.html)
- **[ARDU]** `handle_setup_signing()` écrit la nouvelle clé en stockage puis l'« activate[s] immediately on all links » — il n'y a donc **pas** de période de recouvrement entre ancienne et nouvelle clé — [`GCS_Signing.cpp`, ArduPilot master](https://raw.githubusercontent.com/ArduPilot/ardupilot/master/libraries/GCS_MAVLink/GCS_Signing.cpp)

### Inferences
- **Pas de révocation ⇒ la compromission est permanente jusqu'à intervention physique sur chaque nœud.** Avec une clé partagée de flotte, un seul véhicule perdu/volé impose une re-provision de l'ensemble de la flotte. *(inférence)*
- **Pas de recouvrement de clés** (une seule clé active, activée immédiatement sur tous les liens) ⇒ la rotation est une **opération atomique et désynchronisante** : tant que la GCS et le véhicule n'ont pas la même clé, le lien de commande est mort. En flotte, cela impose une fenêtre de maintenance au sol. *(inférence)*
- L'« appairage » MAVLink revient à une **hypothèse de canal sûr** (USB), sans preuve de possession ni attestation. La sécurité du secret repose entièrement sur la discipline opérationnelle. *(inférence : absence constatée dans la spec)*

### Gaps
- Aucune source officielle ne documente de **procédure de rotation de flotte** (ordre des opérations, gestion de l'horodatage initial lors d'une rotation, effet sur les tables `signing_streams` des récepteurs).
- Aucun mécanisme de **dérivation de clé par session** (type Noise/KK, DH éphémère) n'est mentionné nulle part dans la spec MAVLink consultée.

---

## Q9. Recommandations officielles sur ce que MAVLink signing **ne couvre pas**

### Takeaway
PX4 est la source officielle la plus explicite : la signature **authentifie** mais **n'autorise
pas** (aucune restriction par commande), **ne chiffre pas**, et **ne résiste pas à l'accès
physique**. La spec MAVLink, elle, ne contient pas de section « limites » : l'absence de modèle de
menace formel est elle-même un constat citable.

### Cited Findings
- **[PX4]** « Message signing authenticates MAVLink frames » et fournit « cryptographic authentication for all MAVLink communication » ; cela couvre l'intégrité mais **pas** le chiffrement — « messages are sent in plaintext » — [MAVLink Security Hardening, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/security_hardening.html)
- **[PX4]** **Autorisation par commande non couverte** : la signature vérifie l'identité mais **ne restreint pas quelles commandes** une partie authentifiée peut envoyer — [MAVLink Security Hardening, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/security_hardening.html)
- **[PX4]** « An eavesdropper **can read** telemetry and commands but **cannot forge** them » — [MAVLink Security Hardening, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/security_hardening.html)
- **[PX4]** **Accès physique non couvert** : la carte SD contenant `/mavlink/mavlink-signing-key.bin` est exposée à l'extraction, la modification ou la suppression — [MAVLink Security Hardening, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/security_hardening.html)
- **[PX4]** Liste explicite de ce que la signature **ne protège pas** : absence de chiffrement du contenu, exposition de la clé par extraction de la carte SD, 4 types de messages restant usurpables, récupération d'une clé perdue nécessitant un accès matériel, absence de rotation automatique — [MAVLink Message Signing, docs.px4.io/main](https://docs.px4.io/main/en/mavlink/message_signing.html)
- **[ARDU]** « This does **NOT** encrypt the data, just merely controls if the autopilot will respond to MAVLink commands » ; USB non protégé — [MAVLink2 Signing, ardupilot.org/dev](https://ardupilot.org/dev/docs/common-MAVLink2-signing.html)
- **[SPEC]** La seule obligation de protection du secret énoncée par la spec est : la clé « must not be exposed via any publicly accessible communication protocol » — [Message Signing, mavlink.io](https://mavlink.io/en/guide/message_signing.html)

### Inferences
Synthèse des limites, par origine *(inférence de regroupement ; chaque élément est appuyé par une citation ci-dessus ou plus haut)* :

| Limite | Origine | Statut |
|---|---|---|
| Pas de confidentialité (texte clair) | **SPEC** (conception) | documentée par PX4 et ArduPilot |
| Pas d'autorisation par commande | **SPEC** (hors périmètre) | documentée explicitement par PX4 |
| Secret symétrique partagé, aucune identité par nœud, aucune non-répudiation | **SPEC** | déduit de `sha256_48(secret_key + …)` |
| MAC tronqué à 48 bits | **SPEC** | 2⁻⁴⁸ par tentative *(calcul)* |
| Pas de révocation, pas de rotation automatique | **SPEC** + implémentations | « no automatic key rotation » (PX4) |
| Pas d'appairage cryptographique (hypothèse de canal sûr) | **SPEC** | absence constatée |
| Anti-rejeu = monotonie stricte, pas de fenêtre de réordonnancement | **SPEC** | déduit des règles d'horodatage |
| Fenêtre de 60 s à l'admission d'un flux inconnu | **SPEC** | « 6 million (one minute) » |
| Table de flux bornée (16 par défaut) | **CODE** | limite d'échelle pour les GCS |
| Déclassement MAVLink 1 non bloqué protocolairement | **SPEC** | seule la politique locale protège |
| Messages exemptés usurpables | **PX4** (4 msg) / **ARDU** (2 msg + USB) | choix d'implémentation divergents |
| Clé au repos non protégée (SD card / FRAM, pas d'élément sécurisé) | **PX4** / **ARDU** | documenté PX4 ; déduit pour ArduPilot |
| Horodatage couplé à l'heure GPS ⇒ surface d'usurpation GNSS | **ARDU** (choix) | ticket #13860, fermé sans correctif protocolaire |
| Horodatage sur disque figé après coupure brutale | **PX4** (choix) | documenté par PX4 |
| Pas de protection contre l'accès USB/physique | **ARDU** explicitement, **PX4** implicitement | documenté |

- **Conclusion transversale** : MAVLink signing est correctement décrit comme un mécanisme
  d'**authentification de trames sur un lien, à secret partagé, opt-in**. Il n'est pas, et ne
  prétend pas être, une infrastructure de clés, un contrôle d'accès, ni un canal confidentiel.

### Gaps
- **La spécification MAVLink elle-même ne comporte pas de section « modèle de menace » ni « limites »** : tout l'énoncé des limites provient de la documentation PX4 et, plus brièvement, du wiki ArduPilot. C'est une asymétrie importante à signaler au rédacteur.
- Je n'ai trouvé **aucun document officiel MAVLink** proposant une couche d'autorisation par commande (type liste de permissions par identité). Si une telle proposition existe (RFC/PR dans `mavlink/mavlink`), elle n'est pas remontée dans mes recherches.
- Aucun audit cryptographique externe public de MAVLink signing n'a été trouvé pendant cette recherche.

---

## Annexe — Inventaire des sources primaires utilisées

| Source | Type | Identifiant / date |
|---|---|---|
| [mavlink.io — Message Signing (Authentication)](https://mavlink.io/en/guide/message_signing.html) | Spécification | consultée 2026-10-07, pas de version affichée |
| [mavlink.io — Packet Serialization](https://mavlink.io/en/guide/serialization.html) | Spécification | consultée 2026-10-07 |
| [mavlink.io — MAVLink Versions](https://mavlink.io/en/guide/mavlink_version.html) | Spécification | consultée 2026-10-07 |
| [mavlink.io — C Message Signing (mavgen)](https://mavlink.io/en/mavgen_c/message_signing_c.html) | Spécification / guide bibliothèque | consultée 2026-10-07 |
| [`mavlink_helpers.h`, mavlink/c_library_v2](https://raw.githubusercontent.com/mavlink/c_library_v2/master/mavlink_helpers.h) | Code de référence | branche `master`, 2026-10-07 |
| [docs.px4.io — MAVLink Message Signing](https://docs.px4.io/main/en/mavlink/message_signing.html) | Doc officielle PX4 | branche `main`, 2026-10-07 |
| [`docs/en/mavlink/message_signing.md`, PX4-Autopilot](https://raw.githubusercontent.com/PX4/PX4-Autopilot/main/docs/en/mavlink/message_signing.md) | Source de la doc PX4 | branche `main`, 2026-10-07 |
| [docs.px4.io — MAVLink Security Hardening](https://docs.px4.io/main/en/mavlink/security_hardening.html) | Doc officielle PX4 | branche `main`, 2026-10-07 |
| [ardupilot.org/dev — MAVLink2 Signing](https://ardupilot.org/dev/docs/common-MAVLink2-signing.html) | Doc officielle ArduPilot | consultée 2026-10-07 |
| [`libraries/GCS_MAVLink/GCS_Signing.cpp`, ArduPilot](https://raw.githubusercontent.com/ArduPilot/ardupilot/master/libraries/GCS_MAVLink/GCS_Signing.cpp) | Code ArduPilot (8 440 octets) | branche `master`, 2026-10-07 |
| [ArduPilot issue #13860](https://github.com/ArduPilot/ardupilot/issues/13860) + [fil de commentaires (API)](https://api.github.com/repos/ArduPilot/ardupilot/issues/13860/comments) | Ticket officiel | ouvert 2020-03-22, fermé 2020-03-26 (completed) |

**Sources écartées / à ne pas citer seules** : `discuss.ardupilot.org` et `discuss.px4.io` (forums
communautaires) ; `db.gcve.eu` / CVE-2026-1579 (fiche non récupérée intégralement, voir Q4) ;
`dbugs.ptsecurity.com`, `intel.controlassurance.com` (agrégateurs non consultés).
