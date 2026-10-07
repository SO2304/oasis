# Prompt : protéger la clé et le firmware (passage au RP2350)

À coller dans Claude Code (Opus) à la racine du dépôt OASIS, sur le PC où les cartes
sont branchées. Base : la branche `claude/eloquent-ptolemy-oojjn0`. Lis d'abord :
- `CLAUDE.md` (lignes « Enrollment », « Signed A/B firmware update » et
  « Real MCU hardware boot » de la matrice) ;
- `docs/specs/ENROLLMENT_OWNERSHIP_SPEC.md` et `docs/specs/FIRMWARE_UPDATE_SPEC.md` ;
- `partners/POSITIONING_GAPS.md` ;
- le code de `oasis-silicon-test/` et `oasis-bootloader/`.

---

## Contexte

Sur le RP2040, OASIS a deux trous que tout certificateur ou acheteur sérieux verra,
déjà écrits comme limites dans `CLAUDE.md` :

1. **La clé privée de la carte est lisible.** Elle est stockée en flash, et le mode
   BOOTSEL ou le port SWD suffisent pour la copier. Avec elle, on fabrique des
   ordres « authentiques » depuis n'importe où.
2. **Aucune vérification de l'image au démarrage.** BOOTSEL, SWD ou la commande de
   test `b` permettent de flasher n'importe quel firmware, plancher anti-retour
   compris. La mise à jour signée de la phase 1.3 ne protège donc que le chemin
   prévu.

Le RP2350 (Raspberry Pi Pico 2) annonce dans sa documentation :
- un démarrage sécurisé : l'image est signée et le hachage de la clé publique est
  stocké en OTP ;
- de l'OTP avec verrouillage par page ;
- la désactivation du débogage ;
- TrustZone (Cortex-M33) ;
- des détecteurs de glitch.

**Objectif : sur RP2350, la clé de la carte ne sort jamais de la carte par BOOTSEL ou
SWD, et seule une image signée par le propriétaire démarre, avec anti-retour.**

## Règles non négociables

1. **L'OTP est irréversible. Une erreur détruit la carte.** Avant toute écriture
   d'OTP :
   - écris la séquence exacte, avec les adresses et les valeurs ;
   - simule-la, ou fais-la en lecture seule ;
   - montre-la à l'utilisateur ;
   - **attends son accord explicite, pour chaque carte.**

   Commence sur **une seule carte dédiée aux essais**, jamais sur toutes. Ne verrouille
   jamais le débogage ni BOOTSEL avant d'avoir prouvé qu'une image signée démarre
   **et** qu'une mise à jour signée fonctionne sur cette carte.

   **Si l'utilisateur n'a qu'une carte RP2350**, fais les phases 0 et 1 sans aucune
   écriture d'OTP. Avant la phase 2, rappelle-lui deux choses et laisse-le décider :
   - c'est sa seule carte : une erreur d'OTP la rend inutilisable ;
   - une fois le démarrage sécurisé actif, **perdre la clé de signature empêche tout
     nouveau firmware**. Exige une copie de sauvegarde de cette clé, hors de la
     machine, avant l'écriture du hachage.
2. **Les faits sur le RP2350 viennent de sources primaires** : datasheet RP2350, notes
   d'errata, documentation du bootrom et de `picotool`, publications de Raspberry Pi
   sur la sécurité. Cite la section et la révision. Si une information manque,
   écris « non documenté ».
3. **Cherche et cite les attaques publiées** contre la sécurité du RP2350 : résultats
   du concours de sécurité organisé par Raspberry Pi, injections de fautes, errata
   liés à la sécurité, révision de silicium concernée. Écris ce qu'elles permettent
   et quelle révision les corrige. Indique la révision de silicium des cartes
   utilisées.
4. **Pas de crypto maison.** Uniquement :
   - le démarrage sécurisé du bootrom, avec son algorithme de signature tel que
     documenté ;
   - les primitives déjà dans le dépôt.

   Toute nouvelle crate est justifiée : mainteneur, téléchargements, `no_std`,
   licence compatible avec `deny.toml`.
5. **Mêmes règles que les prompts précédents** :
   - `cargo test --workspace --release`, clippy et fmt passent avant et après ;
   - le build MCU de `CLAUDE.md` passe ;
   - `kani_shards.sh --check` passe ;
   - chaque harnais Kani est vérifié avec une contre-épreuve ;
   - pas de chiffre de performance isolé : K=10, médiane ± demi-écart ;
   - chaque limite est écrite ;
   - commits sur `claude/eloquent-ptolemy-oojjn0`, avec les lignes d'attribution,
     **aucun identifiant de modèle** dans le dépôt.
6. **Le RP2040 reste supporté.** Le code commun ne doit pas régresser. Le RP2040 garde
   ses limites, écrites telles quelles.

## Phase 0 — décision écrite (avant tout code)

Écris `docs/specs/KEY_PROTECTION_SPEC.md` §0, avec une comparaison sourcée des
options :

| Option | Quoi |
|---|---|
| **A** | RP2350 seul : démarrage sécurisé, clé (ou clé d'enveloppement) en OTP, page verrouillée après lecture au démarrage, débogage désactivé |
| **B** | RP2350 + TrustZone : la clé n'est lisible que du monde sécurisé, l'application signe par un appel au monde sécurisé |
| **C** | Élément sécurisé externe : NXP SE050 (vérifie s'il signe en Ed25519), Microchip ATECC608B (P-256 seulement, ce qui veut dire changer d'algorithme ou d'architecture) |

Pour chaque option, donne :
- ce qu'elle protège : extraction par BOOTSEL, par SWD, par firmware malveillant
  flashé, par attaque physique ;
- le coût par carte ;
- l'effort ;
- l'impact sur v0B, ML-DSA et la mise à jour A/B ;
- ce qui reste ouvert.

**Recommande une option, et commence par la plus simple qui ferme BOOTSEL et SWD.**

## Phase 1 — portage RP2350 sans sécurité activée

0. Relève d'abord la révision de silicium de la carte, avec l'outil officiel
   (`picotool info` ou équivalent documenté). Note-la dans le rapport, et indique les
   errata de sécurité qui la concernent (règle 3).

1. Construis `oasis-silicon-test` et `oasis-bootloader` pour RP2350 (Cortex-M33,
   `thumbv8m.main-none-eabihf`). Utilise les versions d'`embassy-rp` /
   `embassy-boot-rp` qui supportent le RP2350, et justifie les versions retenues.
2. Fais tourner la suite T0–T6 et un relais v0B A→B→C sur RP2350, en mixant RP2040
   et RP2350 si le matériel le permet.
3. Mesure K=10 sur Cortex-M33 : signature v0B, vérification v0B, porte hybride
   ML-DSA-44. Compare avec les chiffres RP2040 de `CLAUDE.md`.

## Phase 2 — démarrage sécurisé et anti-retour (carte d'essai uniquement)

1. Signe les images avec l'outillage officiel (`picotool` ou équivalent documenté).
   - La **clé de signature du propriétaire** reste hors de la carte, sur le PC, hors
     du dépôt. Écris la procédure de garde de cette clé.
   - Elle est distincte de la clé de la carte et des clés de propriétaire de la
     phase 1.2. Explique leur rôle respectif.
2. Après accord de l'utilisateur (règle 1), écris le hachage de la clé et active le
   démarrage sécurisé.
3. **Ce que tu dois prouver**, chaque cas avec son log :
   - une image signée démarre ;
   - une image non signée ne démarre pas ;
   - une image signée par une autre clé ne démarre pas ;
   - une image signée mais modifiée d'un octet ne démarre pas.
4. **L'anti-retour.** Utilise le mécanisme de version de rollback du bootrom, s'il
   existe tel que documenté. Une image signée plus ancienne doit être refusée, y
   compris flashée **par BOOTSEL**. Relie-le au plancher de la phase 1.3, en une
   seule source de vérité, et explique laquelle.
5. **La mise à jour A/B signée de la phase 1.3 doit continuer de fonctionner.**
   Coupure de courant pendant l'échange comprise : rejoue le test de la phase 1.3.

## Phase 3 — la clé de la carte ne sort plus

1. Option retenue en phase 0. Si c'est A ou B :
   - la graine Ed25519 (ou une clé qui l'enveloppe) est générée sur la carte, comme
     en phase 1.2 ;
   - elle est stockée en OTP, ou en flash chiffrée par une clé OTP ;
   - la page OTP est verrouillée en lecture dès que le code sécurisé l'a lue au
     démarrage, avant de donner la main à l'application.
2. Désactive le débogage, après accord (règle 1).
3. **Ce que tu dois prouver**, chaque cas avec son log :
   - par SWD : rien de lisible, ou accès refusé ;
   - par BOOTSEL : la lecture de la flash ne contient pas la clé en clair ;
   - un firmware signé mais sans le droit de lire la page ne peut pas lire la clé ;
   - la carte signe toujours v0B ;
   - l'enrôlement de la phase 1.2 fonctionne toujours, preuve de possession
     comprise ;
   - le transfert de propriété fonctionne toujours.
4. **Kani**, chacun avec sa contre-épreuve :
   - la fonction pure de séquence de démarrage : aucune branche ne donne la main à
     l'application avec la page de clé encore lisible ;
   - le choix « signer ou refuser » reste total.

## Phase 4 — documents

1. Termine `KEY_PROTECTION_SPEC.md` : modèle de menace, ce qui est fermé, ce qui
   reste ouvert, avec les attaques physiques publiées de la règle 3.
2. Mets à jour `CLAUDE.md`, en recomptant avec les commandes du fichier :
   - une ligne de matrice « Key protection and secure boot (RP2350) », avec ses
     limites ;
   - la limite « key readable from flash » réécrite pour dire où elle tient encore
     (RP2040) ;
   - le décompte de menaces, si une menace est réellement fermée.
3. Mets à jour `partners/POSITIONING_GAPS.md` et `partners/DIFFERENTIATORS.md`, sans
   rien promettre au-delà des preuves.
4. Rapport `evidence/silicon/<date>/keyprot/REPORT.md` :
   - révision de silicium des cartes ;
   - chaque écriture OTP faite, sur quelle carte, avec l'accord de l'utilisateur ;
   - chaque attaque tentée et son résultat ;
   - `SHA256SUMS`.

## Livrables attendus

- Les commits des phases 0 à 4, chacun vérifié selon la règle 5.
- Un rapport final, en français simple, qui répond à quatre questions :
  - la clé peut-elle encore sortir de la carte, et par quel moyen ?
  - un firmware non signé peut-il encore démarrer ?
  - une ancienne image peut-elle encore être installée ?
  - qu'est-ce qui reste ouvert face à un attaquant qui a la carte en main, avec du
    matériel ?
