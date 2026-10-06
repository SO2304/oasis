# Prompt : révocation signée, porte d'actionnement, suite d'attaques comparative

À coller dans Claude Code à la racine du dépôt OASIS, sur le PC où les 3 RP2040
sont branchés. Base : la branche qui contient `mesh-v0b` et la PR
`SO2304/oasis#1` (mode strict). Le prompt `MESH_V0B_SILICON_FOLLOWUP.md`
(réserve de compteurs côté émetteur) doit être **terminé avant** : la partie B
s'appuie sur des compteurs qui survivent au redémarrage des deux côtés.

---

## Objectif

Rendre OASIS **seul en tête, preuves à l'appui**, sur trois points que
`partners/COMPETITIVE_ANALYSIS.md` marque encore « non atteint » :

- **E. Révocation** : exclure un nœud compromis de tout le mesh sans renouveler
  les clés des autres. La révocation doit être signée par l'opérateur, propagée
  par le mesh, appliquée par chaque relais et persistée.
- **F. Porte d'actionnement** : un actionneur ne bouge que si l'ordre est
  authentifié, frais, émis par une origine autorisée et non révoquée, **et** si
  la porte R14 l'autorise. Preuve formelle de cette règle.
- **« De loin »** : une suite d'attaques publique, **exécutée** contre OASIS,
  Bluetooth Mesh et Reticulum, avec les logs bruts. On ne compare plus des
  documentations : on compare des résultats.

Comme pour v0B : **plus sûr, pas plus rapide**. La performance est mesurée et
rapportée, jamais optimisée au détriment de la sécurité.

## Règles non négociables

1. **Aucune donnée inventée.** Chaque résultat, pour OASIS comme pour un
   concurrent, vient d'un log brut archivé. Si une attaque n'a pas pu être
   exécutée contre un concurrent, écris « non exécuté » et la raison.
2. **Comparaison loyale.** Chaque concurrent est configuré avec **ses réglages
   de sécurité les plus forts documentés** (par exemple : Bluetooth Mesh avec
   AppKey et NetKey distinctes, Reticulum avec un `Link` et l'identification de
   l'initiateur). Un concurrent mal configuré ne prouve rien. Cite la version
   ou le commit exact de chaque concurrent.
3. **Même attaquant pour tous.** Définis une fois les capacités de l'attaquant
   (externe sans clé ; interne avec la clé d'un nœud capturé ; relais
   malveillant sur le chemin) et applique-les à chaque système.
4. **Rapporte aussi ce que les concurrents font mieux.** Débit, anonymat,
   maturité, certification. Un rapport sans aucune défaite d'OASIS ne sera pas cru.
5. Pas de crypto maison : Ed25519 via `ed25519-compact`, SHA-256 via `sha2`. Le
   code reste `no_std` pour tout ce qui tourne sur la puce.
6. **Ne modifie pas** le comportement de v0B ni des versions précédentes. Les
   tests existants restent verts (`cargo test --workspace --release`).
7. Traçabilité des preuves (leçons des trois derniers runs) : firmware compilé
   depuis un arbre **commité et propre**, logs en LF, `SHA256SUMS` vérifié
   **dans un clone frais après le commit**, un numéro de séquence par paquet
   côté émetteur et côté récepteur.
8. Branche `oasis-e-f-attacks`, commits petits et atomiques.

## Ce qui existe déjà (vérifie-le avant de t'en servir)

- `oasis-rt/src/spore_crypto.rs` : `RevocationList` (alias `BTreeSet` en
  `no_std`), enveloppe de révocation signée `SPORE\x06` (`REV_MAGIC = "OASREV"`,
  `parse_revocation_envelope`). Aujourd'hui elle sert la couche `spore`, **pas**
  le mesh, et sa gestion globale dans `spore.rs` est `std` uniquement.
- `oasis-operator-key/` : autorité opérateur, simple ou multisignature k-sur-n
  (`single_from_seed`, `multisig_from_seeds`, `verify_authorization`,
  `apply_transition`).
- `oasis-rt/src/mesh.rs` : `MeshRouter::revoke()` et `is_revoked()` en v0B, mais
  c'est une liste **locale** : non signée, non propagée, non persistée.
- `oasis-rt/src/hyper_state.rs` : `is_action_safe(agent, threshold)`, la porte
  R14 (seuil strict, monotone, déterministe : voir ses tests et preuves Kani).
- `oasis-rt/src/hal.rs` : `clamp_command`, `PhysicalConstraints`, `KillSwitch`.
- `oasis-rt/src/bin/drone_bridge.rs` et `mavlink_adapter.rs` : le pont vers PX4.

## Partie A : spécification (avant tout code)

Écris `docs/REVOCATION_AND_ACTUATION_SPEC.md` :

**Révocation (E)**
- Format d'un message de révocation porté par le mesh (un nouveau type de
  contenu v0B, ou une enveloppe dédiée : justifie). Contenu signé : séparation de
  domaine `"OASIS-REVOKE-v1"`, `network_id`, **numéro d'époque** monotone de la
  liste, empreintes révoquées, horodatage. Signature par la clé opérateur, avec
  option k-sur-n via `oasis-operator-key`.
- Règles au relais : on n'accepte qu'une époque **strictement supérieure** à
  celle connue (pas de retour en arrière, pas de rejeu d'une vieille liste) ; on
  vérifie la signature opérateur **avant** de toucher à l'état ; on persiste
  l'époque et la liste en flash **avant** de relayer ; on rediffuse une seule
  fois par époque.
- Une origine révoquée est rejetée **avant** la vérification de signature du
  message (comme aujourd'hui en v0B), sur **tous** les relais qui ont reçu la
  révocation.
- Ce que la révocation ne couvre pas : un relais déconnecté au moment de la
  diffusion, et la clé opérateur elle-même. Décris le mécanisme de rattrapage
  (par exemple : chaque nœud annonce son époque, un voisin plus à jour lui renvoie
  la liste).

**Porte d'actionnement (F)**
- Un ordre d'actionnement est un message v0B dont le contenu suit un format
  défini (identifiant d'actionneur, consigne, échéance d'expiration, numéro
  d'ordre). Il n'est exécuté que si **toutes** ces conditions sont vraies :
  1. la vérification v0B complète réussit (origine, contenu, fraîcheur, réseau) ;
  2. l'origine figure dans une liste d'**autorités de commande** pour cet
     actionneur (toutes les origines du mesh ne peuvent pas commander) ;
  3. l'origine n'est pas révoquée ;
  4. l'ordre n'a pas expiré (horloge locale monotone, tolérance documentée) ;
  5. `is_action_safe(agent, threshold)` est vrai (R14) ;
  6. la consigne passe `clamp_command` (limites physiques).
- Si une seule condition échoue : **aucune action**, une raison loggée, et un
  compteur de rejets par raison. Aucune action partielle.
- Écris la règle comme une fonction pure, sans effet de bord, par exemple
  `actuation_decision(...) -> Decision`, pour qu'elle soit prouvable par Kani.

**Arrête-toi et montre-moi la spec avant de coder.**

## Partie B : implémentation et preuves (PC)

Tests obligatoires (préfixe `rev_` et `act_`), un par attaque :

| Attaque | Résultat attendu |
|---|---|
| Révocation signée par une clé qui n'est pas celle de l'opérateur | Rejetée, état inchangé |
| Rejeu d'une ancienne liste (époque inférieure ou égale) | Rejetée |
| Liste dont un octet est modifié | Rejetée |
| Révocation valide, puis message de l'origine révoquée | Rejeté au premier relais, avant la signature |
| Révocation valide, redémarrage du relais, puis message de l'origine révoquée | Rejeté (époque et liste persistées) |
| Relais déconnecté pendant la diffusion, puis reconnecté | Rattrape l'époque, puis rejette l'origine révoquée |
| Nœud révoqué qui tente de diffuser une « contre-révocation » | Rejetée (pas la clé opérateur) |
| Ordre d'actionnement d'une origine non autorisée pour cet actionneur | Aucune action |
| Ordre valide mais R14 défavorable (capteur perdu) | Aucune action |
| Ordre valide mais expiré | Aucune action |
| Ordre rejoué | Aucune action |
| Ordre dont la consigne dépasse les limites physiques | Aucune action, ou consigne bornée selon la spec : choisis et justifie |
| Ordre valide, R14 favorable, origine autorisée | Action exécutée, une seule fois |

Preuves Kani (`kani::proof`), au minimum :
- `actuation_decision` ne renvoie « agir » que si les six conditions sont vraies
  (preuve sur des booléens et valeurs symboliques : pas besoin de passer par
  Ed25519, donne en entrée le résultat de la vérification) ;
- R14 défavorable implique « ne pas agir », quelles que soient les autres entrées ;
- l'époque de révocation ne recule jamais ;
- un message de révocation rejeté ne modifie pas l'état.

## Partie C : silicium (3 RP2040)

Étends `uart_mesh.rs` sans casser les commandes existantes. Envoie `K` (mode
strict v0B) à chaque carte avant chaque test.
1. **Révocation propagée** : la clé opérateur (simulée côté PC, envoyée par A)
   révoque l'empreinte de C. B et C reçoivent et persistent l'époque. C tente
   ensuite d'émettre : B rejette au premier saut. Logge l'époque à chaque étape.
2. **Révocation après coupure** : coupe B (demande-la-moi), rallume ; B rejette
   toujours C, avec l'époque relue en flash.
3. **Rejeu d'une vieille liste** : A renvoie l'époque précédente ; B et C la
   rejettent.
4. **Porte d'actionnement** : C est l'actionneur (une LED ou une broche GPIO
   observable). A envoie un ordre valide : la LED s'allume. Puis, dans l'ordre :
   ordre de B (non autorisé), ordre expiré, ordre rejoué, et ordre valide avec un
   capteur simulé perdu (R14 défavorable). La LED ne doit **jamais** s'allumer
   dans ces quatre cas. Logge chaque décision avec sa raison et l'état de la broche.

## Partie D : suite d'attaques comparative

Crée `attack-suite/` à la racine :
- un **catalogue** `attack-suite/ATTACKS.md` : pour chaque attaque, le modèle
  d'attaquant, les étapes, et le critère de succès de l'attaquant ;
- un **adaptateur par système**, et un script qui rejoue le catalogue et écrit
  un log brut par attaque.

Systèmes, du plus prioritaire au moins prioritaire :
1. **OASIS v0B** (PC, puis silicium pour les attaques déjà couvertes).
2. **Reticulum** (Python, `pip install rns` à une version fixée, plusieurs
   instances locales reliées par `UDPInterface` ou `TCPInterface`). Utilise sa
   configuration la plus forte : `Link` avec identification de l'initiateur.
3. **Bluetooth Mesh de Zephyr** en simulation (`native_sim` ou BabbleSim, comme
   les tests de Zephyr), commit Zephyr fixé.
4. Optionnel : **Meshtastic** en build Linux natif (`meshtasticd`) si
   l'installation tient en moins d'une heure ; sinon « non exécuté » et la raison.

Attaques minimales, appliquées à chaque système où elles ont un sens :
| # | Attaque | Attaquant |
|---|---|---|
| 1 | Forger un message au nom d'un autre nœud | Interne (clé d'un nœud capturé) |
| 2 | Modifier le contenu en transit | Relais malveillant |
| 3 | Rejouer un message capturé, immédiatement | Externe |
| 4 | Rejouer un message capturé après redémarrage du relais | Externe |
| 5 | Faire disparaître un message légitime (pollution du cache anti-rejeu ou anti-doublon) | Interne ou relais |
| 6 | Continuer à émettre après révocation | Nœud révoqué |
| 7 | Faire exécuter un ordre d'actionnement non autorisé | Interne | 
| 8 | Inonder le réseau de faux messages : jusqu'où vont-ils ? (nombre de sauts parcourus avant rejet) | Externe et interne |

Pour chaque couple système × attaque, le résultat est l'un de : **bloqué au
premier saut**, **bloqué à l'arrivée**, **réussi**, **non applicable** (avec la
raison), **non exécuté** (avec la raison). Archive le log brut correspondant.

## Livrables

- `docs/REVOCATION_AND_ACTUATION_SPEC.md`.
- Code, tests et preuves Kani ; `cargo test --workspace --release` vert.
- `evidence/silicon/<date>/` : logs, `SHA256SUMS` vérifié dans un clone frais, `REPORT.md`.
- `attack-suite/` : catalogue, adaptateurs, scripts, logs, et
  `attack-suite/RESULTS.md` : le tableau système × attaque, chaque case pointant
  vers son log, avec les versions exactes des concurrents.
- Mets à jour `partners/COMPETITIVE_ANALYSIS.md` et `docs/SECURITY_COMPARISON.md`
  avec les résultats **exécutés**, et retire les mentions « non vérifié » qui
  ont été tranchées.
- Un paragraphe final honnête : sur quels axes OASIS est seul en tête, où il est
  à égalité, où il perd.

## Quand t'arrêter pour me demander

- Après la spec de la partie A.
- Si une attaque ne peut pas être bloquée sans modifier v0B.
- Avant chaque coupure de courant.
- Si l'installation d'un concurrent dépasse une heure : propose de le marquer
  « non exécuté » plutôt que de bricoler une configuration douteuse.
- Si un résultat contre un concurrent semble trop beau : vérifie d'abord que sa
  configuration est bien la plus forte documentée.
