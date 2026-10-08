# Prompt : passerelle pilote Modbus TCP + journal 1.1.9

À coller dans Claude Code (Opus) à la racine du dépôt OASIS. Base : la branche
`claude/eloquent-ptolemy-oojjn0`. Lis d'abord `CLAUDE.md`,
`docs/specs/MODBUS_TCP_SPEC.md`, `docs/specs/MODBUS_GATEWAY_SPEC.md`,
`docs/REVOCATION_AND_ACTUATION_SPEC.md` (partie F), `partners/POSITIONING_GAPS.md`
(ligne A1) et `oasis-rt/src/modbus_tcp.rs`.

---

## Contexte

Le Règlement Machines (UE) 2023/1230 s'applique le 20 janvier 2027. Son annexe III
1.1.9 exige deux choses :
- protéger contre la corruption accidentelle et intentionnelle ;
- **garder une trace des interventions légitimes et illégitimes**.

L'audit commercial du 2026-10-07 a trouvé trois manques qui empêchent tout pilote
payant devant une machine réelle :
1. pas de Modbus TCP ;
2. rien côté opérateur pour signer les ordres d'une IHM inchangée ;
3. pas de journal des interventions.

La partie pure du point 1 est faite (`modbus_tcp.rs` : 7 tests, 2 harnais Kani
vérifiés). **Objectif : une passerelle qu'un intégrateur installe en une journée
entre une IHM et un automate Modbus TCP existants, sans toucher ni l'une ni l'autre,
et qui produit le journal exigé par 1.1.9.**

## Règles non négociables

1. **La porte ne change pas.** Toute écriture passe par
   `modbus_gateway::gateway_decision` (via `modbus_tcp::decide_tcp`). Aucun autre
   chemin ne doit pouvoir écrire sur la socket de l'automate. Il y a **un seul
   endroit** dans le code qui écrit vers l'automate, et il ne reçoit qu'un
   `TcpFrame` issu d'un `Act`.
2. **Pas de crypto maison.** Uniquement ce que le dépôt utilise déjà : v0B / Ed25519,
   SHA-256 (`sha2`), ML-DSA-44 pour les messages d'autorité. Justifie toute nouvelle
   crate : mainteneur, téléchargements, `no_std` ou non, licence compatible avec
   `deny.toml`.
3. **Rust uniquement**, aucun Python (règle du dépôt). Pour simuler l'automate :
   `rmodbus` 0.12.2, déjà utilisé pour les tests.
4. Avant **et** après chaque commit :
   - `cargo test --workspace --release`
   - `cargo clippy` sans nouvel avertissement
   - `cargo fmt --all -- --check`
   - le build MCU de `CLAUDE.md`
   - `oasis-rt/kani_shards.sh --check`
5. **Chaque nouveau harnais Kani** est vérifié, avec une **contre-épreuve** : une
   mutation qui doit le faire échouer. Une contre-épreuve sans effet signifie que le
   harnais est trop faible : renforce-le. Consigne les deux résultats dans
   `evidence/kani/<date>/<sujet>/`.
6. **Pas de chiffre de performance isolé.** K=10, médiane ± demi-écart (règle 5 de
   `CLAUDE.md`).
7. **Honnêteté.** Chaque limite est écrite dans la spec et dans `CLAUDE.md`. N'écris
   jamais « conforme EN 50742 » ni « certifié ».
8. Commits sur `claude/eloquent-ptolemy-oojjn0`, avec les lignes d'attribution de la
   session. **Aucun identifiant de modèle** dans le dépôt. Pas de fusion dans `main`
   sans demande explicite.

## Phase A — les binaires réseau (manques 1 et 2)

1. **`oasis_mbtcp_gateway`** (std, Linux, `std::net`, threads bloquants ; pas de
   runtime asynchrone sauf justification mesurée) :
   - écoute les enveloppes v0B de l'agent (TCP, trames préfixées par leur longueur,
     taille maximale bornée) ;
   - vérifie, décide, et n'écrit vers l'automate que le `TcpFrame` d'un `Act` ;
   - attend la réponse avec un délai d'attente, la vérifie (`check_tcp_response`),
     et renvoie le résultat à l'agent dans une réponse **signée** v0B ;
   - configuration dans un fichier : adresse de l'automate, unité, carte des
     registres et plages, clés enrôlées, réseau ;
   - les clés suivent le modèle d'enrôlement de la phase 1.2. Pas de seed compilé.
2. **`oasis_mbtcp_agent`** :
   - écoute l'IHM en Modbus TCP ;
   - pour chaque écriture : `parse_tcp_write`, puis `to_order`, signature v0B,
     envoi à la passerelle, attente du résultat signé, puis `hmi_response` ;
   - séquence strictement croissante, persistée (même idée que `tx_lease`) ;
   - échéance prise dans l'horloge de la passerelle (point 3) ;
   - un délai d'attente donne `Outcome::NoAnswer`, donc l'exception 0x0B. **Jamais
     de silence vers l'IHM.**
3. **Le temps de la passerelle** :
   - la passerelle publie un balisage signé `OTM1` (`boot_id`, `now_ms`) ;
   - l'agent ne signe aucun ordre sans un balisage frais ;
   - après un redémarrage de la passerelle, les ordres de l'ancien démarrage sont
     refusés. **Teste-le.**
4. **Les lectures (FC03/FC04)** : décide et justifie dans la spec, entre un relais
   direct agent → automate (une lecture ne change pas l'état) et un relais par la
   passerelle (un seul chemin réseau vers l'automate). Le choix par défaut doit
   laisser **un seul** équipement parler à l'automate. Les lectures n'utilisent
   jamais le chemin d'écriture.
5. **Tests d'intégration sur de vraies sockets** (localhost) : IHM simulée → agent →
   passerelle → automate `rmodbus` en TCP. Reprends au minimum les attaques de
   `mbtcp_end_to_end_hmi_agent_gateway_plc` :
   - rejeu ;
   - origine forgée ;
   - valeur modifiée dans une enveloppe authentique ;
   - registre hors carte ;
   - valeur hors plage ;
   - écriture brute emballée ;
   - ordre expiré ;
   - ordre d'un démarrage précédent ;
   - **connexion directe d'un tiers au port de la passerelle avec du Modbus en clair**.

   Un compteur côté automate prouve que **seules les décisions `Act` l'atteignent**.
6. Mesure K=10 de la latence ajoutée (IHM → réponse) en localhost, à côté d'un
   accès direct à l'automate. Donne la médiane ± la demi-écart.

## Phase B — journal des interventions (manque 3, exigence 1.1.9)

1. **Module pur `intervention_log`** (`no_std`) :
   - un enregistrement par décision, contenant :
     - n° d'enregistrement ;
     - `boot_id` et `now_ms` de la passerelle ;
     - empreinte de l'origine ;
     - SHA-256 de l'ordre ;
     - décision et raison (et règle) ;
     - pour un `Act` : le résultat de l'automate (Ack, exception, pas de réponse) ;
   - les **refus avant la porte** sont aussi journalisés : enveloppe invalide, non
     enrôlé, Modbus brut ;
   - **chaînage** : chaque enregistrement contient le SHA-256 du précédent ;
   - **point de contrôle signé** Ed25519 par la passerelle tous les N enregistrements
     et à l'arrêt propre.
2. **Ce qui doit être détecté**, chaque cas par un test :
   - suppression d'un enregistrement ;
   - modification d'un octet ;
   - réordonnancement ;
   - troncature de la fin après le dernier point de contrôle (dire honnêtement ce
     qui est détectable et ce qui ne l'est pas) ;
   - journal remplacé par un autre journal valide d'une autre passerelle.
3. Stockage en fichier, en ajout seulement, avec `fsync` aux points de contrôle.
   Politique de rotation écrite. Coupure de courant pendant une écriture : le
   journal reste vérifiable jusqu'au dernier enregistrement complet. Teste-le par une
   écriture tronquée.
4. **Outil `oasis_log_verify`** : vérifie un journal et l'exporte en CSV, pour le
   dossier technique CE (horodatage, origine, registre, valeur, décision).
5. **Kani** :
   - aucune décision n'est sans enregistrement : l'écriture vers l'automate n'est
     possible qu'après l'ajout au journal ;
   - le vérificateur est total sur toute entrée bornée.

   Chacun avec sa contre-épreuve.
6. **Limite à écrire telle quelle** : sur une machine Linux, un administrateur peut
   effacer le fichier. Le chaînage et les points de contrôle **détectent** la
   falsification, ils ne l'**empêchent** pas. Pour l'empêcher, il faut un export
   périodique hors de la machine, ou un stockage matériel. Dis lequel est fait.

## Phase C — documents et correspondance

1. Mets à jour `docs/specs/MODBUS_TCP_SPEC.md`, puis crée
   `docs/specs/INTERVENTION_LOG_SPEC.md` avec : formats, menaces couvertes, limites.
2. Écris `partners/MAPPING_1_1_9.md`. Pour chaque phrase de l'annexe III 1.1.9 et
   1.2.1 : le mécanisme OASIS, la preuve (test, harnais, log silicium), et ce qui
   reste à la charge du fabricant. **Ne cite pas l'EN 50742 sans le texte** : si la
   FprEN n'a pas été fournie, écris « correspondance EN 50742 à faire quand le texte
   sera disponible ».
3. Mets à jour `CLAUDE.md`, en recomptant avec les commandes du fichier : tests,
   harnais, modules, binaires, fichiers. Ajoute une ligne à la matrice de validation,
   avec ses limites.
4. Guide d'installation pour un intégrateur (`docs/PILOT_INSTALL.md`) :
   - matériel conseillé, avec la limite : un Linux standard ne protège pas sa clé ;
   - schéma réseau ;
   - fichier de configuration commenté ;
   - procédure d'enrôlement ;
   - vérification avant mise en service ;
   - comment lire le journal.

## Livrables attendus

- Les commits des phases A, B et C, chacun vérifié selon la règle 4.
- Un rapport final, en français simple, contenant :
  - ce qui est prouvé (tests, harnais Kani, contre-épreuves) ;
  - ce qui ne l'est pas ;
  - la latence mesurée K=10 ;
  - la liste exacte des limites à dire à un client pilote.
