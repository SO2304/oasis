# Cycle de vie des clés — OASIS

**Écrit le 2026-10-08.** Ferme `POSITIONING_GAPS.md` **C6**. Décrit ce qui existe, avec sa
preuve, et ce qui manque, avec sa raison. C'est la première question d'un RSSI et elle
mérite une réponse qui ne se dérobe pas.

> **Aucune revendication de sûreté de fonctionnement.** Voir
> [`compliance/IEC_TS_63074.md`](compliance/IEC_TS_63074.md).

---

## 1. Les quatre clés du système

| Clé | Où elle vit | Qui la détient | Ce qu'elle autorise |
|---|---|---|---|
| **Identité de nœud** (Ed25519, 32 o de graine) | flash du nœud, secteur `0x1F_8000` | le nœud, et personne d'autre | signer ses propres trames v0B ; être reconnue comme origine |
| **Clé de propriétaire** (Ed25519 + ML-DSA-44) | PC de l'opérateur | l'opérateur | enrôler, révoquer, transférer la propriété, signer un manifeste de micrologiciel |
| **Clé de lien** (HMAC, 32 o) | dérivée, jamais stockée ni transmise | les deux extrémités du lien | le pré-filtre v0C, avant toute vérification Ed25519 |
| **Clé de session AEAD** (ChaCha20-Poly1305) | couche spore uniquement (v3→v7) | les pairs appairés | confidentialité **hors du mesh** — v0B et v0C ne chiffrent pas |

La clé de lien n'est pas une clé de plus à gérer, et c'est délibéré : elle vaut
`HKDF-SHA256(X25519(sk_nous, pk_pair), "OASIS-LINK-v0C" ‖ fp_min ‖ fp_max)`, donc elle est
**dérivée des identités**. Aucune distribution, aucune époque, aucun emplacement flash,
aucune rotation — et un nœud volé ne livre que **ses** liens. C'est la décision
d'architecture validée le 2026-10-07 (`docs/specs/RELAY_PREFILTER_SPEC.md`).

---

## 2. Génération — sur la carte, au premier démarrage

Il n'y a **aucune injection de clé en fabrication**, et c'est un choix : une clé qui ne naît
pas sur la carte a été vue par quelqu'un d'autre.

Au premier démarrage, `oasis_rt::identity` :

1. échantillonne **4 096 bits** de l'oscillateur en anneau (`RAW_SAMPLES = 4096`) ;
2. applique les tests de santé **SP 800-90B** sur l'échantillon brut — répétition
   (`RCT_CUTOFF = 41`) et proportion adaptative (`APT_WINDOW = 1024`,
   `APT_CUTOFF = 793`) ; un échantillon qui échoue est jeté et l'opération recommence,
   **3 fois au plus** (`KEYGEN_ATTEMPTS = 3`), après quoi le nœud reste **sans identité
   utilisable** plutôt que de porter une clé douteuse ;
3. conditionne par **SHA-256** vers une graine de 32 octets ;
4. **refuse une graine inutilisable** (`seed_usable`), dont la graine tout-à-zéro ;
5. persiste la graine et marque l'identité.

Mesuré sur silicium (phase 1.2, `evidence/silicon/2026-10-06/enroll/`) : **3 clés distinctes
au premier démarrage de 3 cartes**, ROSC à 43–47 % de uns, entropie min MCV
**0,81–0,91 bit/échantillon**.

> ⚠️ **L'oscillateur en anneau du RP2040 n'est pas une source d'entropie validée.** La
> datasheet le dit elle-même (§2.17.5). Les tests de santé détectent une panne franche, pas
> un biais subtil ni une attaque active sur l'horloge. Pour un produit, il faut un élément
> sécurisé avec un générateur certifié.

### Le mélange de nonce, une fois

Avant l'enrôlement, l'outil PC envoie un défi et un nonce ; le nœud mélange **une fois** le
nonce de l'outil dans sa graine (`rekey_identity`), persiste et redémarre. Objet : qu'une
clé née d'un ROSC faible ne soit pas la seule entrée. Après l'enrôlement, ce chemin est
fermé — `@E` sur un nœud déjà enrôlé **imprime** son identité au lieu de la renouveler.

**Ce verrou a une conséquence que j'ai failli déclencher** : appeler `@E` sur un nœud absent
de son **propre** registre renouvelle sa clé et détruit son identité enrôlée. Vérifier
`@W` avant.

### Preuve de possession

Le nœud signe le défi de l'outil (`pop_sign`), l'outil vérifie que la clé publique présentée
correspond bien à l'empreinte annoncée (`oasis_enroll verify-pop` → `POP_OK fp=…`, code 0).
Un opérateur n'enrôle donc jamais une clé publique dont le nœud ne détient pas la privée.

---

## 3. Enrôlement — une attestation signée par le propriétaire

Message `OAU1` **kind 2** : `node_pk ‖ role ‖ permissions ‖ enroll_seq`, signé par le
propriétaire courant. L'empreinte est **dérivée** de la clé publique, jamais transmise
séparément.

| Règle | Appliquée où |
|---|---|
| `enroll_seq` **strictement croissant** par nœud | `enrollment::apply` |
| bits de permission inconnus **refusés** (`ReservedPermission`) | masque `perm::KNOWN` |
| un nœud **révoqué n'est jamais enrôlé** | `enrollment` |
| un nœud non enrôlé est « unknown sender » **au premier saut** | `mesh::process_v0b` |

Permissions, délibérément séparées : **`ACTUATE`** (bit 0) agir, **`STOP`** (bit 1) arrêter,
**`SUPERVISE`** (bit 2) émettre une balise de supervision. Un nœud qui peut bouger une
machine **n'obtient pas l'arrêt** — donc le verrou, donc un déni de service — gratuitement ;
et un superviseur peut arrêter sans pouvoir agir.

Démontré le 2026-10-08 (`evidence/silicon/2026-10-08/hardening/`) : B passée à `perms=1`
(`ACTUATE` seul) voit son ordre d'arrêt refusé `NotAuthorized` **sans poser le verrou**,
puis restaurée à `perms=7`.

L'attestation circule comme n'importe quel message d'autorité : fragmentée sur le mesh
(`@F`), ou appliquée localement (`@L`) — et **l'USB n'est pas un canal privilégié**, le
message passe par la même porte. Coût mesuré d'une vérification Ed25519 d'attestation sur
RP2040 : **181 ms**.

---

## 4. Propriété — transfert à deux signatures

Un transfert exige **deux** signatures : une **offre** signée par le propriétaire courant,
et une **acceptation** signée par les clés offertes, **liée au condensé de l'offre**. Un
second transfert exige une nouvelle signature (`NeedsResign`), et sans offre en cours
l'acceptation est refusée (`NoPendingOffer`).

Démontré sur 3 cartes (phase 1.2) : transfert o1 → o2, acceptation d'un tiers (o3) refusée,
messages de l'ancien propriétaire refusés ensuite, et après une **coupure de courant réelle**
la carte a retrouvé sa clé, son propriétaire o2, ses pairs, et a revérifié la liste de
révocation avec o2.

> ⚠️ **La propriété est par réseau, pas par appareil.** Transférer la propriété d'un parc
> transfère tout le parc d'un coup. Un déploiement multi-client a besoin d'un `network_id`
> par client.

---

## 5. Révocation — permanente, par époque, appliquée par chaque relais

`ORV1`, ou `OAU1` kind 1 en hybride depuis la phase 1.1 : signé par l'opérateur (une
signature, ou **k-sur-n** via `oasis-operator-key`), **époque strictement croissante**,
**permanente** (chaque liste est un sur-ensemble de la précédente), **persistée avant
application**, **transmise une fois par époque**, avec balises de rattrapage.

Démontré (phase E/F) : propagée A→B→C ; une origine révoquée est abandonnée **au premier
saut, avant même sa vérification de signature** ; une liste plus ancienne est refusée
(`Rollback`) ; la liste est restaurée depuis la flash après une coupure de courant réelle.

Trois limites, toutes assumées :

- un nœud révoqué peut encore **relayer** le trafic des autres ;
- le **rattrapage** et le **k-sur-n** sont **PC uniquement** ;
- la révocation est **permanente par conception** : il n'y a pas de « dé-révocation ». Une
  révocation par erreur se répare en **réenrôlant une nouvelle clé** sur le nœud, pas en
  réhabilitant l'ancienne. C'est aussi pourquoi le test « un arrêt d'une origine révoquée
  est refusé » n'a pas été lancé sur les cartes de développement : il les retirerait du
  parc définitivement.

---

## 6. Rotation — ce qui n'existe pas

| Clé | Rotation | État |
|---|---|---|
| Identité de nœud | **avant enrôlement seulement** (mélange du nonce, une fois) | ❌ aucune rotation après enrôlement |
| Clé de lien | **sans objet** — dérivée des identités, donc elle tourne quand une identité change | ✅ par construction |
| Clé de propriétaire | par **transfert de propriété** vers un nouveau couple de clés | 🟡 c'est un transfert, pas une rotation : l'ancienne clé est oubliée, pas révoquée |
| Clé de session AEAD | par appairage (couche spore) | 🟡 hors mesh |

**Il n'y a donc pas de rotation de clé de nœud.** Le chemin existant est : révoquer
l'ancienne empreinte, générer une nouvelle identité sur la carte, la réenrôler avec un
`enroll_seq` supérieur. Trois messages d'autorité au lieu d'un, et la révocation étant
permanente, l'ancienne empreinte occupe une place définitive dans la liste
(`MAX_REVOKED = 16` sur le nœud — **c'est un plafond dur** : 16 rotations épuisent le parc).

C'est le manque le plus structurant de ce document. Il n'est pas fermé ici.

---

## 7. Stockage de la clé opérateur

**Aujourd'hui : aucune intégration HSM.** Les graines de test (`o1` = `0x0E/0x0F`,
`o2` = `0x2E/0x2F`, `o3` = `0x3E/0x3F`) sont **en clair dans
`oasis-operator-key/examples/oasis_enroll.rs`** (fonction `owner()`), et les fichiers
voisins qui portent les mêmes graines le disent : « *Simulated operator seed (test only;
never compiled into firmware)* » (`ef_payloads.rs`, `pq_payloads.rs`). Elles ne doivent
jamais servir en production, et **rien dans le dépôt ne l'empêche techniquement**.

Ce qui existe pour réduire le risque :

- **k-sur-n** : une liste de révocation peut exiger `k` signatures sur `n` porteurs
  (`oasis-operator-key`, `no_std`), donc la perte ou le vol d'**une** clé ne suffit ni à
  révoquer ni à bloquer. ⚠️ Testé **sur PC uniquement**.
- la clé opérateur **ne touche jamais un nœud** : les cartes ne voient que des clés
  publiques et des messages signés.

Ce qui manque : un adaptateur vers un porteur matériel (PKCS#11, carte à puce, YubiKey), une
procédure de cérémonie de clés, et une sauvegarde répartie. Sans cela, `POSITIONING_GAPS.md`
cite à juste titre « compromission de la clé opérateur » parmi les menaces **non couvertes**.

---

## 8. Perte et récupération

| Perte | Conséquence | Récupération |
|---|---|---|
| **Clé d'un nœud** (carte détruite) | ce nœud ne peut plus émettre | révoquer son empreinte, enrôler une carte neuve. Les autres nœuds **ne renouvellent rien** |
| **Clé d'un nœud compromise** (lue en flash) | l'attaquant peut signer comme ce nœud, et détient ses clés de lien | révocation — appliquée au premier saut, prouvée sur silicium |
| **Clé de propriétaire perdue** | ❌ **plus aucun enrôlement, révocation, transfert ni mise à jour signée.** Le parc continue de fonctionner, mais il est **figé** | **aucune**, sauf si le k-sur-n était en place avec des porteurs survivants |
| **Clé de propriétaire compromise** | l'attaquant enrôle, révoque, transfère et signe des micrologiciels | ❌ aucun mécanisme. Un transfert de propriété exige la clé courante, donc l'attaquant l'a aussi |
| **Flash d'un nœud effacée** | identité et enrôlement perdus | la carte régénère une clé au démarrage et doit être **réenrôlée** |

**Les deux lignes rouges sont la perte et la compromission de la clé de propriétaire.** Elles
sont la raison d'être du k-sur-n, et le k-sur-n n'est pas prouvé sur silicium.

---

## 9. Tableau de correspondance

| Exigence | Où | Couverture |
|---|---|---|
| IEC 62443-4-2 **CR 1.5** *Authenticator management* | enrôlement, `enroll_seq`, transfert de propriété | 🟡 pas de rotation (§6) |
| IEC 62443-4-2 **CR 1.8** *PKI certificates* | — | ➖ **non applicable, à justifier** : OASIS n'utilise pas de X.509, mais des attestations signées par le propriétaire |
| IEC 62443-4-2 **CR 1.9 + RE(1)** *Strength / hardware security of public-key auth* | Ed25519 | 🟡 / ❌ **RE(1) absent** : clé lisible en flash |
| IEC 62443-4-2 **CR 4.2** *Information persistence* | — | ❌ aucun effacement sûr des secrets |
| IEC 62443-4-2 **EDR 3.12 / 3.13** *Provisioning roots of trust* | racine « propriétaire », transférable | 🟡 pas de racine « fournisseur » distincte |
| EN 18031-1 **SSM** *stockage sécurisé* | — | ❌ ne se ferme pas en logiciel (`POSITIONING_GAPS.md` **C14**) |
| EN 18031-1 **CCK** *confidentialité des clés* | née sur la carte, ne sort jamais | 🟡 pas protégée d'un accès physique |
| CRA annexe I-I-2 **m)** *suppression sûre et permanente* | — | ❌ |

## 10. Les cinq manques, nommés

1. **Aucune rotation de clé de nœud** après enrôlement, et `MAX_REVOKED = 16` plafonne les
   remplacements (§6). *Le plus structurant.*
2. **Clé de nœud lisible en flash** par BOOTSEL ou SWD : pas d'élément sécurisé. Ne se
   ferme pas en logiciel (**C14**).
3. **Aucune récupération après perte de la clé de propriétaire**, et le k-sur-n qui
   l'atténue n'est prouvé que sur PC (§7, §8).
4. **Aucun stockage matériel de la clé opérateur**, et les graines de test sont en clair
   dans le dépôt (§7).
5. **Aucun effacement sûr des secrets** (CRA annexe I-I-2 m), IEC 62443-4-2 CR 4.2).

Aucun de ces cinq n'est fermé par ce document. Il les nomme pour qu'on cesse de les
redécouvrir en entretien.
