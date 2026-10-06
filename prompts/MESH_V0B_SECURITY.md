# Prompt : mesh v0B, plus sûr que Reticulum et Meshtastic (pas plus rapide)

À coller dans Claude Code à la racine du dépôt OASIS, sur le PC où les 3 RP2040
sont branchés (la phase 3 a besoin des cartes).

---

## Mission

Tu implémentes **v0B** (`SPORE\x0B`), une nouvelle enveloppe mesh d'OASIS. Elle
doit être **plus sûre** que ce qui existe, **pas plus rapide**. La performance
n'est pas un objectif : tu la mesures et tu la rapportes, sans l'optimiser au
détriment de la sécurité.

L'objectif, en une phrase vérifiable :

> Chaque relais vérifie l'origine, le contenu, la fraîcheur et le réseau de
> chaque message. Un message forgé, modifié, rejoué ou venant d'un autre réseau
> est rejeté au premier saut, même après un redémarrage du relais.

## Contexte : pourquoi, et face à quoi

Lis d'abord `CLAUDE.md`, `oasis-rt/src/mesh.rs`, `oasis-rt/src/mesh/tests.rs`,
`oasis-rt/src/mesh/kani_proofs.rs`, `oasis-rt/src/spore_crypto.rs` (fenêtre
anti-rejeu 128 bits de v7), `evidence/silicon/2026-10-04/REPORT.md` §12b et
`evidence/silicon/2026-10-06/REPORT.md`.

**Ce que font les concurrents (vérifié dans le code de Reticulum, commit `e40191b`) :**
- Reticulum : les relais **ne vérifient rien de cryptographique sur les paquets
  de données**. Ils transmettent selon la table de chemins, le nombre de sauts et
  un anti-doublon (`RNS/Transport.py`, l.2018-2045 et 2123-2160). Seules les
  annonces (`validate_announce`) et les preuves sont vérifiées. Les paquets n'ont
  pas d'adresse source, par choix d'anonymat. Un faux trafic traverse donc tout
  le réseau avant d'être rejeté à l'arrivée.
- Meshtastic : les canaux utilisent une clé partagée (AES-CTR). Seuls les messages
  directs et l'administration sont signés en Ed25519 (depuis la version 2.5), et
  vérifiés à l'arrivée.
- Thread et Zigbee : une clé réseau partagée par tous les nœuds.

**Les défauts actuels de v0A, à corriger dans v0B (constatés dans le code) :**
1. **Le contenu n'est pas signé.** `mesh_v10_sign` signe `magic || msg_id ||
   origin_fp` (22 octets). Sur silicium, 12 inversions de bit sur 50 dans le
   contenu ont été acceptées (Phase 4). Pire : un relais malveillant peut garder
   la signature, remplacer le contenu, et le faux message est mémorisé comme vu.
   Le vrai message, qui a le même `msg_id`, est alors **écarté comme doublon**.
   C'est une attaque par suppression de messages.
2. **Pas de fraîcheur vérifiable.** `msg_id = origin_msg_id(fp, tx_counter)` est
   un hash : le récepteur ne peut pas vérifier que le compteur augmente.
   L'anti-rejeu repose uniquement sur le cache en RAM (4096 entrées + Bloom). Un
   vieux message signé est donc **accepté à nouveau** après un redémarrage du
   relais, une remise à zéro du Bloom ou un débordement du cache.
3. **Pas de séparation de domaine.** La signature n'inclut aucun identifiant de
   réseau : un message d'un réseau A est rejouable sur un réseau B où l'origine
   est enregistrée.
4. **La révocation n'est pas appliquée par les relais** (à vérifier dans le code) :
   une origine révoquée doit être rejetée au premier saut, pas seulement à l'arrivée.

## Règles non négociables

- **Ne modifie pas** le comportement de v8, v9 et v0A : ajoute v0B à côté. Les
  444 tests existants doivent rester verts (`cargo test --workspace --release`).
- **Pas de crypto maison.** Ed25519 via `ed25519-compact` (déjà utilisé), SHA-256
  ou SHA-512 via les crates déjà présentes. Aucune nouvelle primitive.
- **`no_std`** : v0B doit compiler pour `thumbv6m-none-eabi` avec
  `--no-default-features --features mesh_bloom_mcu,mesh_v10` (ou un nouveau
  feature `mesh_v11` si c'est plus propre).
- **Vérifier avant de mémoriser** : un message n'entre dans le cache de doublons
  et ne met à jour le compteur de son origine **qu'après** une vérification
  complète réussie. Un message invalide ne doit jamais influencer l'état du relais.
- **Aucun chiffre sans log brut.** Si un test échoue, écris FAIL et la cause. Ne
  modifie jamais un test pour le faire passer.
- Travaille sur une branche `mesh-v0b`, commits petits et atomiques.

## Phase 1 : spécification (avant le code)

Écris `docs/MESH_V0B_SPEC.md`, une page :
- **Format d'enveloppe** : magic `SPORE\x0B`, `network_id` (8 octets), `origin_fp`
  (8), `counter` (8, en clair et signé), `ttl` (1), `hops` (2), `payload_len` (2),
  signature (64), payload. `msg_id` = dérivé de (`origin_fp`, `counter`) comme
  aujourd'hui, ou supprimé si `counter` le remplace : justifie ton choix.
- **Données signées**, avec séparation de domaine explicite :
  `"OASIS-MESH-v0B" || network_id || origin_fp || counter || payload_len || SHA-256(payload)`.
- **Champs non signés** : `ttl` et `hops`, qui changent à chaque saut. Écris ce
  qu'un relais malveillant peut en faire et comment c'est borné (TTL plafonné à
  `MAX_TTL` à la réception, rejet si `hops > MAX_TTL`, rejet si `ttl + hops > MAX_TTL`).
- **Ordre des vérifications au relais**, de la moins chère à la plus chère :
  taille et magic → `network_id` → origine connue et non révoquée → compteur
  au-dessus de la fenêtre de l'origine → signature Ed25519 → mise à jour de la
  fenêtre et du cache → retransmission.
- **Fraîcheur** : pour chaque origine, un compteur maximal vu et une fenêtre
  glissante de 128 bits (réutilise la logique de v7 dans `spore_crypto.rs`).
  Ce compteur maximal est **persisté**. Rejet de tout compteur trop ancien ou
  déjà vu dans la fenêtre.
- **Modèle de menace** : un tableau attaque → défense → test qui le prouve.

**Arrête-toi et montre-moi la spec avant de coder.**

## Phase 2 : implémentation et tests sur PC

Tests obligatoires, un par attaque, nommés `v0b_*` :

| Attaque | Résultat attendu |
|---|---|
| Inversion de 1 bit dans **chaque** octet de l'enveloppe signée, un par un (boucle exhaustive) | Rejet, avec la raison attendue |
| Contenu remplacé par un relais, signature conservée | Rejet au relais, et le vrai message passe encore |
| Origine usurpée (signée avec une autre clé) | Rejet « bad signature » |
| Rejeu immédiat | Rejet « duplicate / stale counter » |
| Rejeu **après un redémarrage du relais** (état restauré depuis la persistance) | Rejet |
| Rejeu après remise à zéro du Bloom et débordement du cache de doublons | Rejet |
| Message d'un autre `network_id` | Rejet avant la vérification de signature |
| Origine révoquée | Rejet avant la vérification de signature |
| `ttl` gonflé par un relais | Plafonné, aucune amplification au-delà de `MAX_TTL` |
| Message invalide suivi du vrai message | Le vrai message est accepté : l'état n'a pas été pollué |
| Réordonnancement dans la fenêtre (jusqu'à 127 positions) | Accepté une seule fois |

Ajoute des preuves Kani dans `src/mesh/kani_proofs.rs`, au minimum :
- le parseur v0B ne panique jamais, quelle que soit l'entrée ;
- un message rejeté ne modifie ni la fenêtre ni le cache ;
- la fenêtre ne recule jamais (monotonie) ;
- le contenu fait partie des données signées : modifier un octet du contenu change les données signées.

Mesure le coût sur PC (K=10, médiane ± demi-écart) de la signature et de la
vérification v0B contre v0A. Rapporte-le. N'optimise pas.

## Phase 3 : preuve sur silicium (3 RP2040)

Étends `oasis-silicon-test` (nouvelle commande, sans casser T0–T6 ni `uart_mesh.rs`) :
1. **Phase 4 rejouée en v0B** sur la chaîne UART A → B → C : 50 inversions de bit
   avant CRC. Objectif : **0 accepté**, contre 12/50 en v0A. Logge la raison de
   chaque rejet.
2. **Remplacement de contenu par B** (relais malveillant) : C rejette, et le vrai
   message de A arrive quand même à C par un second envoi.
3. **Rejeu après coupure de courant** : A envoie, B relaie ; on coupe et
   rallume B (vraie coupure USB, demande-la-moi) ; A rejoue le vieux message.
   B doit le rejeter grâce au compteur persisté en flash. C'est aussi le **T8**
   du prompt silicium initial.
4. **Coût sur la puce** : temps de signature et de vérification v0B (minuterie
   matérielle, K=5), comparés à v0A (≈174 ms / 178 ms avec une paire de clés en cache ; 341 ms si la clé est recalculée à chaque appel).

Archive les logs bruts dans `evidence/silicon/<date>/`, avec `SHA256SUMS` calculé
**après** l'écriture finale des fichiers (fins de ligne LF) et vérifié par
`sha256sum -c`. Compile le firmware depuis un arbre **commité** : le hash git
présent dans les logs doit désigner le code qui a tourné.

## Phase 4 : comparatif

Écris `docs/SECURITY_COMPARISON.md` : un tableau des scénarios d'attaque
ci-dessus, avec le comportement d'OASIS v0B, de Reticulum, de Meshtastic et de
Thread. Règles :
- pour OASIS, cite le test ou le log qui le prouve ;
- pour chaque concurrent, cite le fichier et la ligne de son code, ou la page de
  sa documentation ; si tu ne peux pas le vérifier, écris « non vérifié » ;
- écris aussi ce qu'OASIS **paie** pour cette sécurité : pas d'anonymat de
  l'émetteur, et environ 176 ms de vérification par saut sur Cortex-M0+.

## Quand t'arrêter pour me demander

- Après la spec de la phase 1.
- Si une attaque du tableau ne peut pas être bloquée sans modifier v0A ou sans
  primitive cryptographique nouvelle.
- Si la persistance du compteur en flash risque d'user la flash : propose une
  stratégie, par exemple réserver des blocs de compteurs en avance.
- Si un test échoue deux fois avec la même cause sans que tu trouves pourquoi.

## Rapport final attendu

- Le tableau attaque → résultat sur PC → résultat sur silicium, avec les logs.
- Les coûts v0B contre v0A, sur PC et sur la puce.
- Les limites qui restent (extraction de clé sur RP2040, canaux auxiliaires,
  post-quantique), écrites telles quelles.
