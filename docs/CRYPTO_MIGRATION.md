# Plan de migration cryptographique

**Écrit le 2026-10-08.** Ferme **A6** de [`partners/POSITIONING_GAPS.md`](../partners/POSITIONING_GAPS.md).
Branche `positioning-alignment`.

> **Ce document ne revendique aucune fonction de sûreté.** OASIS n'est pas une fonction de
> sûreté certifiée (ni PL au sens de l'ISO 13849-1, ni SIL au sens de l'IEC 62061) et ne
> réduit aucun risque machine. Voir [`compliance/IEC_TS_63074.md`](compliance/IEC_TS_63074.md).

La phase 3 du prompt demandait, pour A6, **« un identifiant d'algorithme dans l'enveloppe
*ou* un plan de migration écrit, sans casser v0B »**. Ce document livre le second, et
explique pourquoi c'est le bon des deux : l'identifiant existe déjà là où il peut être
vérifié avant usage (les messages d'autorité), et l'ajouter à l'enveloppe par saut
créerait exactement l'amplificateur de déni de service que la phase 2.1 vient de fermer
(§4).

Les tailles et le budget radio ne sont pas rappelés ici en détail : ils sont établis à la
source dans [`compliance/PQC.md`](compliance/PQC.md), que ce plan prolonge.

---

## 1. Inventaire : chaque primitive, et si elle est remplaçable aujourd'hui

« Agile » = le format porte un identifiant d'algorithme **et** un mécanisme refuse le
déclassement. « Versionnée » = l'algorithme est figé par un numéro de version qu'on peut
réallouer, sans négociation en ligne. « Figée » = ni l'un ni l'autre.

| Couche | Primitive | Identifiant | État | Où |
|---|---|---|---|---|
| **Autorité** (révocation, politique, enrôlement, transfert, manifeste) | Ed25519 **ou** hybride Ed25519 + ML-DSA-44 | **octet de suite signé** (`SUITE_ED25519`/`SUITE_HYBRID`), minimum par type persisté en deux emplacements, jamais abaissé, déclassement refusé **avant** toute vérification | ✅ **agile** | `authority.rs`, `docs/specs/PQ_AUTHORITY_SPEC.md` |
| **Par saut** v0B | Ed25519 sur `domaine‖network_id‖origin_fp‖compteur‖payload_len‖SHA-256(charge)` | le magic `SPORE\x0B` | 🟡 **versionnée** | `mesh.rs` |
| **Pré-filtre de lien** v0C | HMAC-SHA256 tronqué à 16 o, clé `HKDF-SHA256(X25519(sk, pk), "OASIS-LINK-v0C"‖fp_min‖fp_max)` | le magic `SPORE\x0C` | 🟡 **versionnée** | `mesh/prefilter.rs` |
| **Spore** v3→v7 | ChaCha20-Poly1305, X25519 (DH éphémère par message en v4/v5/v7) | le magic `SPORE\x03`…`\x07` | 🟡 **versionnée** | `spore_crypto.rs` |
| **Ordre à deux signatures** `OAQ1` | Ed25519, k parmi n | **aucun** | ❌ **figée** | `quorum.rs` (§6-d) |
| **Journal de décisions** | chaîne SHA-256 | aucun | ❌ **figée** | `journal.rs` |
| **Identité de nœud** | Ed25519 (signature) + X25519 (spore) | aucun | ❌ **figée** | `identity.rs` |
| **Signature MAVLink v2** | HMAC-SHA-256 tronqué à 48 bits | fixé par le standard MAVLink | ❌ hors de notre main | `mavlink_min.rs`, `compliance/MAVLINK_SIGNING_GAP.md` |

Deux lectures de ce tableau :

1. **La correction d'A6 tient** : « pas d'agilité cryptographique » est faux des messages
   d'autorité et vrai partout ailleurs. C'est déjà écrit en F.1 de `POSITIONING_GAPS.md`.
2. **Il n'existe aucun KEM post-quantique dans OASIS.** L'établissement de clé est X25519
   partout. C'est l'écart réel avec Veridify
   ([`competition/VERIDIFY.md`](competition/VERIDIFY.md)), et il n'est pas fermé par ce plan.

---

## 2. Tailles ML-KEM — vérifiées à la source

Manquaient à `PQC.md`, qui ne couvrait que les signatures. **FIPS 203, Table 3**, « Sizes
(in bytes) of keys and ciphertexts of ML-KEM », publié le **13 août 2024**
([PDF](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.203.pdf)) :

| Jeu | Clé d'encapsulation (o) | Clé de décapsulation (o) | **Chiffré (o)** | Secret partagé (o) |
|---|---:|---:|---:|---:|
| ML-KEM-512 | 800 | 1 632 | **768** | 32 |
| ML-KEM-768 | 1 184 | 2 400 | **1 088** | 32 |
| ML-KEM-1024 | 1 568 | 3 168 | **1 568** | 32 |

Catégories, verbatim du même document (§8, après la Table 2) : « *Concretely, ML-KEM-512
is claimed to be in security category 1, ML-KEM-768 is claimed to be in security
category 3, and ML-KEM-1024 is claimed to be in security category 5.* » Le BSI n'acceptant
que les catégories 3 et 5 (`PQC.md` §1), **le choix conforme est ML-KEM-768 : 1 088 octets
de chiffré, 1 184 de clé publique.** À comparer aux 32 octets d'une clé publique X25519.

### Ce que ça change selon l'endroit

**[calcul]**, avec le budget établi dans `PQC.md` §3 (36 s/heure/bande, SF12 → 51 o
applicatifs par trame) :

| Emplacement du KEM | Fréquence | Coût ML-KEM-768 | Verdict |
|---|---|---|---|
| **Clé de lien du pré-filtre** | **une fois par pair**, puis en cache (`LINK_CACHE_SLOTS = 8`) | 1 088 o = 22 trames ≈ **1,7 h** de budget, une seule fois | **envisageable** hors ligne ou à SF7 (5 trames, 2,0 s, soit **5,6 %** du budget horaire) |
| **DH éphémère de spore v4/v5/v7** | **par message** | 1,7 h de budget **par message** | **exclu** |

Ces quatre chiffres, et ceux du §6-d, sont reproduits et **assertés** par
les tests de `budget` dans `oasis-lora-transport` (`cargo test`), qui recalculent au passage la valeur publiée par
`PQC.md` §4 (48 trames, 3,72 h pour une signature ML-DSA-44) pour vérifier que les deux
documents comptent de la même façon : une trame pleine = `CAP[SF]` octets applicatifs dans
une charge PHY qui porte en plus **13 octets** de surcoût LoRaWAN. ⚠️ Ce surcoût de 13 o
est **non vérifié à la source** (TS001 non relu), comme la formule de time on air
elle-même.

La leçon de la phase 2.1 s'applique directement et elle est déjà payée une fois : un
X25519 coûte ~190 ms sur un M0+, autant qu'une vérification Ed25519, et mon pré-filtre a
d'abord dérivé la clé **par trame** (370 ms/trame) avant de la mettre en cache. Un KEM
post-quantique dans un chemin par message est la même erreur, en plus gros.

---

## 3. La contrainte qui décide tout

`PQC.md` §4 l'établit : **une signature post-quantique par saut est impossible sur LoRa**
(ML-DSA-44 = 48 trames à SF12, soit 3,72 h du budget légal horaire ; ML-DSA-65, le choix
conforme BSI, 5,04 h). Ce plan n'a donc **pas** pour but de rendre la couche par saut
post-quantique. Il a pour but de garantir qu'on **peut** changer d'algorithme par saut
sans casser une flotte déployée, et de nommer ce qu'il faudrait changer d'abord pour que
la question du post-quantique par saut se pose (une autre radio, ou une agrégation de
signatures normalisée — qui n'existe pas, `PQC.md` §4).

---

## 4. Le mécanisme retenu : la version, pas la négociation

**Décision : la couche par saut migre par allocation d'un nouveau magic, pas par un octet
de suite dans l'en-tête.** Le magic *est* l'identifiant d'algorithme.

Trois raisons, dans l'ordre de force :

1. **Un octet de suite dans l'en-tête par saut n'est pas authentifié au moment où il sert.**
   Il faudrait le lire pour choisir le vérificateur, donc avant toute vérification. Un
   attaquant le choisit alors librement, et choisit par là le travail cryptographique que
   le relais va faire. C'est précisément le défaut **C1** que la phase 2.1 vient de
   fermer : une trame forgée coûtait 179,4 ms à un relais, 0,70 ms après le pré-filtre
   (silicium, `evidence/silicon/2026-10-07/prefilter/REPORT.md`). Un octet de suite
   négociable rouvrirait ce chemin et l'amplifierait, puisqu'une suite post-quantique
   coûte plus cher qu'une classique.
2. **Le mécanisme anti-déclassement existe déjà et il est prouvé.** Un routeur v0B est
   **strict par défaut** : il refuse v8, v9 et v0A (`mesh.rs`, « legacy envelope rejected
   by strict v0B router »). Sur silicium : 150/150 tracées, 0 acceptée, déclassement
   refusé (`evidence/silicon/2026-10-06/followup/`, empreinte `3a67e6e`). Migrer par
   version réutilise ce mécanisme au lieu d'en inventer un second.
3. **Là où la négociation est appropriée, elle est déjà là.** Les messages d'autorité sont
   rares, fragmentés et en stockage-retransmission : leur octet de suite est **dans la
   zone signée**, le minimum par type est persisté et ne baisse jamais, et le déclassement
   est refusé **avant** tout travail de signature — mesuré à 4 ms sur silicium (phase 1.1).
   La différence avec l'en-tête par saut n'est pas une question de goût : c'est que la
   politique est connue d'avance et stockée localement, donc l'attaquant ne la choisit pas.

### Registre des suites par saut

| Magic | Suite | État |
|---|---|---|
| `SPORE\x0B` | Ed25519 + SHA-256, compteur persisté | **en service**, prouvé sur silicium |
| `SPORE\x0C` | v0B + `forwarder_fp` + HMAC-SHA256-128 de lien + budget | **en service**, prouvé sur silicium |
| `SPORE\x0D` | **réservé** — prochaine suite par saut | **non implémenté**, non alloué à un algorithme |

⚠️ `SPORE\x0D` n'existe pas dans le code. Cette ligne réserve le numéro et rien de plus ;
aucun document OASIS ne doit décrire v0D comme disponible.

---

## 5. Le plan, par étapes, avec critères de sortie

Une flotte ne fait pas de jour J : à tout instant pendant la migration, des nœuds de deux
générations doivent se parler. D'où l'ordre **accepter, puis exiger, puis émettre, puis
refuser** — et non l'inverse.

| # | Étape | Ce qui change | Critère de sortie | Retour arrière |
|---|---|---|---|---|
| **E0** | Registre écrit | ce document | le registre existe et nomme la suite visée | sans objet |
| **E1** | **Accepter** v0D | micrologiciel : `process_v0d` ajouté ; **émission inchangée (v0B/v0C)** | un test prouve qu'une trame v0D est acceptée **et** qu'une v0B l'est encore ; **chaque nœud** rapporte sa liste acceptée | reflasher ; rien n'est persisté |
| **E2** | **Exiger** v0D | un message de politique signé relève le minimum par saut | la politique est relue depuis la flash après coupure sur **tous** les nœuds | **aucun** — voir ci-dessous |
| **E3** | **Émettre** v0D | émission basculée | aucun nœud ne rapporte de v0D refusée | reflasher ; la politique d'E2 reste |
| **E4** | **Refuser** v0B | mode strict sur la nouvelle suite | 150/150 v0B tracées et refusées, comme pour v0A | aucun |

> ⚠️ **E2 est irréversible par construction.** Le minimum de suite ne descend jamais —
> c'est la propriété qui empêche un attaquant de déclasser une flotte, et elle empêche
> aussi l'opérateur de revenir. **E2 ne doit donc venir qu'après qu'E1 est confirmée sur
> chaque nœud, un par un.** Un nœud resté en E0 au moment d'E2 est définitivement exclu de
> la flotte : il faut le reflasher par USB, c'est-à-dire y accéder physiquement.

L'étape E2 n'a pas besoin de toucher v0B. Elle a besoin d'**un** champ : un minimum de
suite par saut, porté par le même message de politique `OAU1` que les autres. Le coût est
d'un emplacement dans `AuthPolicy::min_suite` — et ce coût n'est pas nul, voir §6-a.

---

## 6. Prérequis absents — ce qui bloque réellement

Aucun de ces points n'est fermé par ce document. Ils sont la vraie réponse à « quand
pouvez-vous migrer ».

### a. L'enregistrement de politique n'est pas versionné — et sa relecture échoue vers le bas

`POLICY_RECORD_LEN = 4 + KIND_COUNT + 4` (`authority.rs`), et `AuthPolicy::from_bytes`
refuse tout enregistrement dont la longueur diffère. Donc **ajouter un type de message —
exactement ce que demande E2 — change la longueur de l'enregistrement persisté, et un
nœud mis à jour ne sait plus lire ses deux emplacements flash.** Le repli n'est pas une
erreur : c'est la politique **par défaut**. Toute élévation qu'une flotte avait faite
(révocation en hybride, par exemple) est **silencieusement perdue** au premier démarrage
après la mise à jour.

Pinné par un test, dans le code d'aujourd'hui
(`authority::tests::pq_policy_two_slots_merge_to_the_strongest`) :

```rust
let mut wider = [0u8; POLICY_RECORD_LEN + 1];
wider[..POLICY_RECORD_LEN].copy_from_slice(&new);
assert_eq!(AuthPolicy::from_bytes(&wider), None, "a longer record is unreadable");
assert_eq!(AuthPolicy::from_slots(&wider, &wider), AuthPolicy::default());
assert_ne!(AuthPolicy::from_slots(&wider, &wider), raised, "the raise is lost");
```

**C'est le mécanisme d'agilité qui n'est pas lui-même migrable.** Correctif à faire
**avant** E2, pas pendant : porter le magic à `APL2`, et accepter un enregistrement `APL1`
en l'élargissant (les emplacements absents prennent le défaut). Avec un test qui prouve
qu'un enregistrement `APL1` est encore honoré par un lecteur `APL2`.

### b. Il n'y a pas de rotation de clé de nœud

[`KEY_LIFECYCLE.md`](KEY_LIFECYCLE.md) §6 : aucune rotation après enrôlement, et
`MAX_REVOKED = 16` est un plafond dur (§5 de ce même document). Or **changer d'algorithme de signature change le type de clé** : chaque nœud
a besoin d'une nouvelle paire, et les anciennes devraient être révoquées. Une flotte de
20 nœuds ne peut pas révoquer ses 20 anciennes clés. **Tant que la rotation n'existe pas,
une migration de suite par saut qui change le type de clé n'est pas exécutable sur le
terrain** — elle se fait par reflashage USB de chaque nœud, c'est-à-dire en les touchant
tous.

C'est le prérequis le plus coûteux de la liste et il ne se contourne pas par un document.

### c. Pas de stockage sécurisé

C14. La clé est lisible en flash par BOOTSEL ou SWD. Une clé ML-DSA l'est autant qu'une
clé Ed25519 ; migrer ne ferme pas C14, et C14 ne se ferme pas en logiciel.

### d. `OAQ1` n'a pas d'octet de suite

Le format des ordres à deux signatures (C9, écrit le 2026-10-08) est figé sur Ed25519.
Il est **neuf et n'a jamais tourné sur silicium**, donc le corriger coûte presque rien
aujourd'hui et cher plus tard. Si un acheteur veut des ordres critiques post-quantiques,
c'est ici que ça se décide — et la taille tranche : un ordre à deux signatures fait déjà
**343 octets** contre une limite physique de 255 (`quorum::tests::q_sizes_do_not_fit_one_radio_frame`).
Avec deux signatures ML-DSA-44, **[calcul]** à partir de `oaq1_len(n) = 51 + 1 + 96n` et de
FIPS 204 Table 2 (pk 1 312 o, signature 2 420 o) :

| Encodage | Taille | Trames à SF12 | Budget légal |
|---|---:|---:|---:|
| tel qu'aujourd'hui, clés portées : `51 + 1 + 2 × (1 312 + 2 420)` | **7 516 o** | 148 | **11,5 h** |
| index d'un octet, clés provisionnées : `51 + 1 + 2 × (1 + 2 420)` | **4 894 o** | 96 | **7,4 h** |

Un ordre critique post-quantique par radio longue portée n'existe pas.

### e. Aucun KEM post-quantique

§2. L'établissement de clé reste X25519 : clé de lien du pré-filtre et DH éphémère de
spore. La première est migrable (une fois par pair), la seconde non (par message).

### f. Conflit de catégorie BSI, non mesuré

`PQC.md` §1 : ML-DSA-44 est catégorie 2, le BSI exige ≥ 3. Le couple conforme serait
**ML-DSA-65 (3 309 o) + ML-KEM-768 (1 088 o)**. Sur RP2040, ML-DSA-44 coûte 198 ms et
44,8 Ko de pile (mesuré, phase 1.1) ; **ML-DSA-65 n'a pas été mesuré sur RP2040** et la
pile est la ressource critique (264 Ko de RAM au total).

### g. Rien de tout cela n'a jamais été transmis par radio

Tous les budgets de ce document et de `PQC.md` sont **calculés**. Le pilote SX1262 existe
et est testé contre un mock ; il n'a jamais tourné sur silicium avec une radio.

---

## 7. Ce qui est déjà conforme au jalon UE 2030

Un seul point, et il est réel. Le jalon 2 de la feuille de route UE (31/12/2030) nomme
explicitement les **mises à jour de micrologiciel « quantum-safe activées par défaut »**
(`PQC.md` §1). C'est exactement la phase 1.3 : manifeste `OAU1` kind 3, **hybride exigé
par défaut**, Ed25519 + ML-DSA-44, prouvé sur trois RP2040 avec deux coupures de courant
réelles (`evidence/silicon/2026-10-06/fwupdate/REPORT.md`).

Autrement dit : **le chemin par lequel la migration arriverait sur le terrain est
lui-même déjà post-quantique.** C'est la seule partie du jalon 2030 qu'OASIS satisfait, et
elle est la plus utile à satisfaire en premier.

---

## 8. Décision

1. **La couche par saut reste classique** (Ed25519 + SHA-256 + HMAC de lien) et migre par
   **allocation de version**, jamais par négociation en ligne. `SPORE\x0D` est réservé.
2. **Le post-quantique reste cantonné aux messages rares** — autorité, révocation,
   politique, transfert de propriété, manifeste de micrologiciel — où il est déjà en
   service et prouvé.
3. **Avant toute étape E2, corriger §6-a** (versionner l'enregistrement de politique). Ce
   correctif est petit, local, testable, et sans lui la première migration efface les
   élévations de politique de la flotte.
4. **Le prérequis qui gouverne le calendrier est §6-b**, la rotation de clé de nœud. Le
   dire à un acheteur d'infrastructure critique avant qu'il ne le découvre.
5. **Ne pas promettre d'ordre par saut post-quantique sur LoRa.** Ce n'est pas une
   difficulté d'ingénierie, c'est le budget légal d'émission.

## 9. Limites de ce document

- **Vérifié à la source dans cette session** : FIPS 203 (PDF NIST, Table 3 et le §8 sur
  les catégories, publié le 13/08/2024).
- **Repris de `compliance/PQC.md`**, qui porte ses propres mentions « non vérifié à la
  source » — notamment **la formule de time on air** (datasheet SX1276 inaccessible) :
  **tout chiffre de trames/heure ou d'heures de budget ci-dessus en hérite** et porte
  **[calcul]**.
- **Aucune mesure nouvelle** : rien n'a été exécuté sur silicium pour ce document. Les
  deux seules choses vérifiées par exécution sont le test du §6-a et les assertions de
  `budget::tests` dans `oasis-lora-transport`, toutes deux en logiciel.
- **E1 à E4 n'ont pas été exécutées.** C'est un plan ; aucune de ses étapes n'a tourné,
  même en simulation.
- `SPORE\x0D` n'est associé à **aucun** algorithme : décider lequel demanderait une mesure
  de pile sur RP2040 qui n'existe pas (§6-f).
