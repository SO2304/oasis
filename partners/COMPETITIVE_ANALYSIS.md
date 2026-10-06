# OASIS face aux concurrents, axe par axe (2026-10-06)

Méthode : les affirmations sur OASIS viennent du code (`oasis-rt/src/mesh.rs` sur
`main`) et des logs silicium. Les affirmations sur les concurrents viennent de leur
code ou de leur documentation, sources en bas. « À vérifier » signifie que je n'ai
pas trouvé de source directe.

**Mise à jour du 2026-10-06 (après-midi).** La colonne « OASIS après v0B +
révocation » est désormais **mesurée** pour les axes E et F (branche
`oasis-e-f-attacks`, preuves brutes dans `evidence/silicon/2026-10-06/ef/`). Les
colonnes des concurrents restent issues de la lecture de leur code ou de leur
documentation : la suite d'attaques exécutée (partie D) est **différée en attente
d'environnement de simulation** (`attack-suite/DEFERRED.md`). Aucune comparaison
exécutée n'existe encore, donc aucun « à vérifier » n'a été tranché, et la phrase
« OASIS seul en tête, preuves à l'appui » ne peut pas encore être écrite.

Légende : ✅ fait · ⚠️ partiel ou avec une faiblesse · ❌ absent · — sans objet

## 1. Les axes où OASIS veut gagner

| Axe | OASIS aujourd'hui (v0A) | OASIS après v0B + révocation | Bluetooth Mesh | Thread | Meshtastic | Reticulum | LoRaWAN |
|---|---|---|---|---|---|---|---|
| **A. Chaque relais authentifie l'origine avec une clé propre au nœud** (un nœud capturé ne peut pas se faire passer pour un autre) | ✅ en-tête seulement, prouvé sur silicium | ✅ | ❌ Les relais vérifient le NetMIC avec la clé réseau **partagée** : tout détenteur de la clé peut forger | ❌ Clé réseau partagée | ❌ Les relais ne vérifient pas ; clé de canal partagée ; messages directs vérifiés à l'arrivée seulement | ❌ Aucune vérification cryptographique des données par les relais (`Transport.py` l.2018-2045) | — Pas de relais ; les passerelles retransmettent sans vérifier, le serveur vérifie |
| **B. Chaque relais vérifie l'intégrité du contenu** | ❌ Le contenu n'est pas signé (12/50 inversions acceptées sur silicium) | ✅ Empreinte du contenu signée | ⚠️ Le NetMIC couvre le paquet, mais avec la clé partagée | ⚠️ Idem, clé partagée | ❌ | ❌ | — |
| **C. Anti-rejeu qui survit à un redémarrage** | ❌ Cache de doublons en RAM uniquement : un vieux message signé est réaccepté après redémarrage ou remise à zéro du Bloom | ✅ Compteur signé + fenêtre par origine, persistés | ✅ Numéro de séquence et liste anti-rejeu persistés. ⚠️ Mais un détenteur de la clé réseau peut **saturer la liste anti-rejeu** avec de fausses entrées et bloquer les messages légitimes | ✅ Compteurs de trame (persistance : à vérifier) | ⚠️ Identifiant de paquet de 32 bits et anti-doublon ; persistance non documentée | ⚠️ Liste d'empreintes de paquets **sauvegardée sur disque** et rechargée au démarrage (`Transport.py` l.339-343, 3745-3769), mais non signée : un relais ne peut pas vérifier la fraîcheur | ✅ Compteurs de trame persistés |
| **D. Un nœud compromis ne peut pas bloquer les messages des autres** | ❌ Remplacement du contenu → le vrai message est écarté comme doublon | ✅ Un message n'entre dans l'état du relais qu'après vérification complète | ❌ Saturation de la liste anti-rejeu possible avec la clé réseau | ❌ Clé partagée | ❌ | ⚠️ Rien n'est vérifié en transit ; seule la destination filtre | — |
| **E. Révoquer un seul nœud sans recléfier tout le réseau** | ❌ Révocation signée (v6) côté `spore`, **`std` seulement**, et **non appliquée par les relais mesh** | ✅ **Fait, prouvé sur silicium** : liste signée par l'opérateur (k-sur-n possible), époque strictement croissante, persistée avant application, appliquée dès le premier relais, relue en flash après coupure. ⚠️ Un nœud révoqué peut encore relayer le trafic des autres ; k-sur-n et rattrapage testés sur PC seulement | ⚠️ « Key Refresh » : on renouvelle la clé de tous les nœuds sauf l'exclu. Lourd mais standard | ⚠️ Changement de clé réseau pour tous | ❌ Changement manuel de la clé de canal | ⚠️ Listes « blackhole » locales ou par abonnement, sans révocation globale (choix assumé) | ✅ Désactivation côté serveur (centralisé) |
| **F. Sûreté des actionneurs liée aux communications** (aucune action si capteurs peu fiables, MAVLink/PX4) | ✅ Porte R14, pont MAVLink | ✅ **Fait, prouvé sur silicium** : porte à 7 conditions (v0B, origine autorisée, non révoquée, non expirée dans l'horloge de l'actionneur, R14, limites physiques y compris NaN, numéro d'ordre croissant). Aucun ordre refusé n'a fait monter la broche ; LED non confirmée visuellement | ❌ | ❌ | ❌ | ❌ | ❌ |
| **G. Langage sûr en mémoire + preuves formelles** | ✅ Rust, 113 harnais Kani | ✅ | ⚠️ Implémentations en C (ex. Zephyr), fuzzées et qualifiées SIG | ⚠️ OpenThread en C++, fuzzé et certifié | ❌ C++ | ⚠️ Python ; microReticulum en C++ | ⚠️ C, certifié |
| **H. Preuves brutes publiées sur silicium** | ✅ Logs, empreintes SHA-256, firmware | ✅ | Certification à la place | Certification | ❌ | ❌ | Certification |

## 2. Les axes où OASIS perd, et c'est assumé

| Axe | Qui gagne | Pourquoi on ne se bat pas ici |
|---|---|---|
| Débit et coût par saut | Tous. Ils utilisent AES-CCM ou HMAC (quelques µs) ; OASIS fait une vérification Ed25519 (176 ms par saut sur Cortex-M0+) | C'est le prix de A. On choisit la sûreté. |
| Anonymat de l'émetteur | Reticulum | OASIS doit savoir qui parle pour filtrer au relais : c'est incompatible par construction. |
| Certification, écosystème, radio réelle, énergie | Bluetooth Mesh, Thread, LoRaWAN, Meshtastic | Des années d'avance. OASIS a 0 utilisateur et pas de radio testée. |
| Protection de la clé sur le nœud | Égalité basse : la plupart tournent sur des puces sans protection activée par défaut | À rattraper avec le RP2350 (OTP, secure boot), pas à revendiquer. |

## 3. Comment devenir « injouable » sur A à F

Aucun concurrent ne coche A, B, C, D et E en même temps : soit la clé est partagée
(Bluetooth Mesh, Thread), soit rien n'est vérifié en transit (Reticulum,
Meshtastic). C'est la place à prendre. Dans l'ordre :

1. **v0B** (prompt `prompts/MESH_V0B_SECURITY.md`) : contenu, compteur et
   identifiant de réseau signés ; état modifié seulement après vérification ;
   persistance. Ferme B, C et D.
2. **Révocation au niveau mesh, en `no_std`** : liste signée par la clé
   opérateur, propagée dans le mesh, appliquée par chaque relais avant la
   vérification de signature, persistée. Ferme E sans recléfier le réseau, ce
   qu'aucun concurrent en mesh ne fait.
3. **Lier R14 aux ordres authentifiés** : un actionneur ne bouge que si l'ordre
   est signé, frais, émis par une origine autorisée et non révoquée, **et** si
   R14 l'autorise. Avec une preuve Kani de cette règle. Ça rend F unique et
   difficile à copier, parce que les concurrents sont des réseaux, pas des
   systèmes d'actionnement.
4. **Une suite de tests d'attaque publique et rejouable** (rejeu après
   redémarrage, remplacement de contenu, saturation de l'anti-rejeu, nœud
   révoqué) exécutée contre OASIS **et** contre Bluetooth Mesh (Zephyr) et
   Reticulum, avec les logs. C'est ce qui transforme « on est meilleurs » en
   fait vérifiable.
5. **RP2350** : clé en OTP et secure boot, pour que « un nœud capturé ne
   compromet que lui-même » ne repose pas uniquement sur la signature par nœud.

## Sources

- Reticulum : code source `markqvist/Reticulum`, commit `e40191b` (`RNS/Transport.py`, `docs/source/understanding.rst`) ; [manuel Reticulum (PDF)](https://reticulum.network/manual/Reticulum%20Manual.pdf) ; [Using Reticulum (blackhole)](https://reticulum.network/manual/using.html)
- Bluetooth Mesh : [Mesh Protocol 1.1](https://www.bluetooth.com/wp-content/uploads/Files/Specification/HTML/MshPRT_v1.1/out/en/index-en.html) ; [Mesh Security Overview (SIG)](https://www.bluetooth.com/wp-content/uploads/2025/04/MeshSecurityOverview_INFO_v1.0-1.pdf) ; [Zephyr Bluetooth Mesh Core](https://docs.zephyrproject.org/latest/services/connectivity/bluetooth/api/mesh/core.html) ; [Silicon Labs, Sequence Number and IV Index](https://docs.silabs.com/btmesh/latest/btmesh-iv-update/02-sequence-number-and-iv-index)
- Thread : [Silicon Labs, OpenThread Security](https://docs.silabs.com/openthread/latest/thread-fundamentals/09-security)
- Meshtastic : [Updated Security Implementation](https://meshtastic.org/docs/development/reference/encryption-technical/) ; [Encryption limitations](https://meshtastic.org/docs/about/overview/encryption/limitations/) ; [Mesh Broadcast Algorithm](https://meshtastic.org/docs/overview/mesh-algo/)
- microReticulum : [Reticulum Community Wiki](https://reticulum.miraheze.org/wiki/MicroReticulum)

## 4. microReticulum, le concurrent direct (lu dans son code, 2026-10-06)

Source : `attermann/microReticulum`, commit `40fa628` (2026-07-20), version 0.5.0,
licence Apache-2.0, environ 40 000 lignes de C++ plus ArduinoJson, MsgPack,
`attermann/Crypto` et `microStore`. C'est un portage C++ de Reticulum, compatible
avec lui (tests d'interopérabilité avec Python dans `test_interop/`).

### Là où OASIS se différencie

| Point | microReticulum | OASIS | Preuve OASIS |
|---|---|---|---|
| Vérification des données par chaque relais | ❌ Le relais vérifie qu'il est le prochain saut, puis « Just increase hop count and transmit », sans aucun contrôle cryptographique (`Transport.cpp` l.1980-2000, identique à Reticulum) | ✅ Origine, contenu, fraîcheur et réseau vérifiés à chaque saut (v0B) | 0/150 inversions acceptées sur silicium |
| Anti-rejeu | Liste d'empreintes de paquets persistée (`_packet_hashlist`, `PersistedBytesList`), bornée, non signée | Compteur signé par l'origine, fenêtre persistée, vérifiable par chaque relais ; réserve de compteurs côté émetteur | Rejeu à l'octet près refusé après une coupure, émetteur redémarré accepté |
| Exclure un nœud compromis | Listes « blackhole » locales ou par abonnement | Révocation signée par l'opérateur (k-sur-n possible), à époque croissante, propagée, persistée, appliquée par chaque relais | Silicium : origine révoquée rejetée au premier saut, y compris après coupure |
| Lien avec les actionneurs | Aucun | Porte d'actionnement à 7 conditions (ordre signé, autorité, non révoqué, non expiré, R14, limites, non rejoué) | 3 preuves Kani + LED sur silicium |
| Langage et preuves | C++ ; tests unitaires et d'interopérabilité ; pas de vérification formelle | Rust, 133 harnais Kani, 16 nouveaux exécutés et vérifiés | `evidence/kani/2026-10-06/` |
| Preuves publiées | Pas de logs sur silicium publiés | Logs bruts, firmware et SHA-256 vérifiés dans un clone frais | `evidence/silicon/` |
| Cible | ESP32, nRF52840 (Cortex-M4F) ; un fichier de carte RP2040 (RAK11300) existe, mais aucune cible RP2040 dans `platformio.ini` | Cortex-M0+ sans FPU (RP2040), validé sur 3 cartes | `evidence/silicon/` |

### Là où microReticulum est devant

| Point | microReticulum | OASIS |
|---|---|---|
| Radio LoRa réelle | ✅ Matériel RNode, nombreuses cartes LoRa | ❌ Pilote SX1262 testé sur mock seulement |
| Routage | ✅ Annonces et table de chemins : un paquet suit un chemin | ❌ Inondation avec TTL : chaque paquet occupe tout le réseau, coûteux en temps d'antenne LoRa |
| Écosystème | ✅ Compatible Reticulum : Sideband, NomadNet, LXMF | ❌ Aucun |
| Liens chiffrés et gros transferts | ✅ `Link` avec confidentialité persistante, `Resource` | ⚠️ Couche spore (v4/v5/v7), pas reliée au mesh v0B |
| Anonymat de l'émetteur | ✅ Par conception | ❌ Incompatible avec le filtrage au relais |
| Coût par saut | Quasi nul (pas de vérification) | ≈ 178 ms de vérification Ed25519 sur Cortex-M0+ |

### Non mesuré des deux côtés

- RAM et flash réellement utilisées : microReticulum ne publie pas de chiffres ;
  OASIS ne connaît que des réservations (tas de 160 Ko) et la taille du firmware
  (≈ 136 Ko de flash).
- Consommation d'énergie.
