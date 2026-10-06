# Prompt : dépasser Veridify DOME

À coller dans Claude Code à la racine du dépôt OASIS, sur le PC où les 3 RP2040 sont
branchés. Base : la branche `claude/eloquent-ptolemy-oojjn0`. Lis d'abord
`partners/DIFFERENTIATORS.md`, `partners/MARKET_VALIDATION.md`,
`partners/POSITIONING.md` et `partners/POSITIONING_GAPS.md`.

---

## Contexte

Veridify Security (produit DOME) est le concurrent commercial le plus proche
d'OASIS : authentification d'équipements OT sur microcontrôleurs (STM32, Renesas RA,
RISC-V), sans accélérateur cryptographique, vendue via des fabricants (KMC Controls),
avec enrôlement sans contact, gestion de propriété, mises à jour du firmware et
algorithmes post-quantiques (ML-DSA, ML-KEM, Falcon), en plus de leur algorithme
propriétaire WalnutDSA.

OASIS a deux avantages qu'aucun concurrent ne documente : chaque relais vérifie
l'origine et le contenu avec la clé propre de l'émetteur, et une porte
d'actionnement prouvée par Kani. Veridify a quatre avances : post-quantique,
enrôlement, mises à jour, clients. **Objectif : rattraper les quatre, et rendre nos
deux avantages incontestables.**

## Règles non négociables

1. **Veille concurrentielle sur sources publiques uniquement** : documentation,
   fiches produits, pages partenaires, brevets, publications scientifiques,
   commentaires officiels du NIST, présentations publiques. **Interdit** : tester,
   attaquer ou sonder un produit, un service ou un système de Veridify ; acheter
   ou télécharger leur logiciel pour le décompiler ; les contacter sous un faux
   prétexte. Si une information n'est pas publique, écris « non documenté ».
2. « Non documenté » ne veut pas dire « absent ». Ne transforme jamais une absence
   de documentation en faiblesse affirmée.
3. Toute affirmation sur Veridify est datée et sourcée. Toute affirmation sur
   OASIS renvoie à un test, un log ou une preuve Kani.
4. Pas de crypto maison : uniquement des algorithmes standardisés (FIPS 203/204/205,
   RFC 8032, RFC 7748, RFC 8439) et des implémentations Rust existantes et
   maintenues. Justifie le choix de chaque crate.
5. Mêmes règles que les prompts précédents : `cargo test --workspace --release`
   vert, arbre commité et propre pour le firmware, logs en LF, `SHA256SUMS`
   vérifié dans un clone frais, branche `oasis-vs-veridify`, commits atomiques.

## Phase 0 : veille publique sur Veridify (avant tout code)

Produis `docs/competition/VERIDIFY.md` :

- **Fiche produit** : fonctions, architecture déclarée, cibles matérielles, modèle
  de déploiement (SaaS, local), partenaires, clients publics, prix si publics.
- **Cryptographie** : algorithmes utilisés et pour quoi. Historique public de
  WalnutDSA : attaques de Hart et al. (2018), de Beullens et Blackburn, retrait de
  la compétition post-quantique du NIST, et ce que Veridify a publié depuis.
  Rôle actuel de ML-DSA, ML-KEM et Falcon dans DOME : par défaut, en option, en
  hybride ?
- **Modèle de confiance** : qui détient les clés, rôle de la « chaîne de blocs de
  propriété », dépendance à leur service, fonctionnement hors ligne.
- **Topologie** : authentification d'équipement à équipement, ou aussi multi-saut ?
  Que se passe-t-il quand un équipement intermédiaire est compromis ?
- **Actionnement** : bloquent-ils seulement les ordres non autorisés, ou aussi
  selon l'état de la machine, l'expiration, les limites physiques ?
- **Assurance** : audits, certifications (IEC 62443, FIPS 140-3), preuves formelles,
  code ouvert ou fermé.
- **Tableau final** : pour chaque point, Veridify (documenté / non documenté),
  OASIS (prouvé / conçu / absent), source.

**Arrête-toi et montre-moi la synthèse avant la phase 1.**

## Phase 1 : rattraper les quatre retards

Une spec courte pour chaque point, montrée avant le code.

1. **Post-quantique et agilité cryptographique.**
   - Ajoute un identifiant d'algorithme aux messages d'autorité, sans casser v0B.
   - Signature **hybride** Ed25519 + ML-DSA-44 pour les messages rares et critiques
     (révocation, enrôlement, mise à jour), avec fragmentation signée si le
     message dépasse une trame (une signature ML-DSA-44 fait 2 420 octets).
   - Mesure sur RP2040 : taille du firmware, RAM, temps de vérification ML-DSA-44.
     Si c'est impossible sur Cortex-M0+, dis-le et mesure sur RP2350 si tu en as un.
   - Garde Ed25519 seul pour les ordres fréquents, et documente pourquoi.
2. **Enrôlement et transfert de propriété.**
   - Au premier démarrage, le nœud génère sa clé. Il ne la sort jamais.
   - Le propriétaire signe une attestation d'enrôlement (empreinte, rôle,
     autorisations, réseau). Le transfert de propriété exige la signature de
     l'ancien **et** du nouveau propriétaire.
   - Outil PC minimal d'enrôlement, puis test sur les 3 cartes : enrôlement,
     transfert, refus d'un nœud non enrôlé.
3. **Mise à jour du firmware signée.**
   - Deux emplacements (A/B), image signée, compteur anti-retour persisté (même
     mécanisme que la réserve de compteurs), retour automatique si la nouvelle
     image ne démarre pas.
   - Sur silicium : image valide installée ; image modifiée, ancienne image et
     image d'un autre signataire refusées ; coupure pendant l'écriture sans
     brique.
4. **Équipements existants (brownfield).** Veridify cible aussi les réseaux
   existants. Implémente un **mode passerelle** : un nœud OASIS placé devant un
   équipement Modbus RTU qui n'accepte que les écritures passées par la porte
   d'actionnement. Démontre sur une carte jouant l'équipement Modbus : une écriture
   de registre forgée, rejouée ou non habilitée n'atteint jamais l'équipement.
   C'est la réponse directe au cas FrostyGoop (commandes Modbus non authentifiées).

## Phase 2 : rendre nos avantages incontestables

1. **Vérification à chaque relais** : ajoute le pré-filtre peu coûteux contre la
   saturation par vérification forcée (voir `POSITIONING_GAPS.md` C1), mesure la
   résistance de nos propres relais avant et après, et documente le chiffre.
2. **Preuves formelles** : fais tourner **toute** la suite Kani (pas seulement les
   nouveaux harnais) dans une CI ou sur une machine qui en a la mémoire. Ajoute
   des preuves pour l'enrôlement, la mise à jour (jamais d'image plus ancienne)
   et la passerelle Modbus (aucune écriture sans décision « agir »).
3. **Fuzzing** des parseurs (v0B, ORV1, OAC1, attestation d'enrôlement, en-tête
   de mise à jour, trames Modbus) avec `cargo-fuzz`, et rapport des heures et
   des cas trouvés.
4. **Transparence** : SBOM CycloneDX, `cargo-audit`, `cargo-deny`, `SECURITY.md`
   conforme au CRA.

## Phase 3 : comparaison publique et honnête

Écris `partners/OASIS_VS_VERIDIFY.md`, une page :
- un tableau point par point, chaque cellule avec sa preuve ou sa source ;
- **ce que Veridify fait mieux**, dit clairement (clients, maturité,
  certifications éventuelles, intégrations) ;
- trois phrases qu'un commercial peut utiliser face à un fabricant, chacune
  appuyée sur une preuve ;
- aucune affirmation sur une faiblesse de Veridify qui ne soit pas publique et
  sourcée.

Mets aussi à jour `DIFFERENTIATORS.md`, `POSITIONING.md` et la page technique avec
**uniquement** ce qui est prouvé.

## Quand t'arrêter pour me demander

- Après la synthèse de la phase 0.
- Après chaque spec de la phase 1, avant de coder.
- Avant chaque coupure de courant.
- Si ML-DSA ne tient pas sur le RP2040.
- Si une information sur Veridify semble ne pouvoir être obtenue qu'en dehors des
  sources publiques : n'y va pas, signale-le.
