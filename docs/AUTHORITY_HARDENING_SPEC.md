# Durcissement de l'autorité de commande — spécification (phase 2, segment S1)

**Écrit le 2026-10-07, avant tout code.** Segment retenu : **machines mobiles autonomes**
(`partners/SEGMENT_COMPARISON.md`). Cette spécification couvre les **trois manques bloquants**
de ce segment, dans l'ordre où ils doivent être construits :

| Partie | Manque | Exigé par | Statut |
|---|---|---|---|
| **G** | **C2** — arrêt asymétrique et état sûr sur perte de liaison | annexe III **1.2.1 al. 4 d) et f)** et dernière phrase ; ISO 13850:2015 4.1.1.2 / 4.1.1.4 | à construire |
| **H** | **C12** — vivacité de la supervision | annexe III **partie 3** : « si la fonction de supervision n'est pas active, la machine ne peut pas fonctionner » | à construire |
| **I** | **C5** — journal infalsifiable des décisions | annexe III **1.1.9 al. 5** et **1.2.1 al. 2 f)** ; CRA annexe **I-I-2 l)** ; IEC 62443-4-2 **CR 2.8, 2.9, 2.10, 2.11, 2.12, 3.9** | à construire |

**C6 est fermé** par [`KEY_LIFECYCLE.md`](KEY_LIFECYCLE.md). **C3 est traité en partie J**
ci-dessous (mesure faite, réduction spécifiée). **C4** (émission d'un ordre sur le terrain)
fera l'objet de la section K.

> **Aucune revendication de sûreté de fonctionnement.** Rien ici n'est une fonction de sûreté
> certifiée (PL au sens d'ISO 13849-1, SIL au sens d'IEC 62061). Voir
> [`compliance/IEC_TS_63074.md`](compliance/IEC_TS_63074.md).

---

## 0. La contrainte qui gouverne toute la partie G

ISO 13849-1:2023, verbatim via l'IFA/DGUV :

> « **If operating mode selection enables or disables safety functions, it is treated as a
> safety function in its own right.** »

Il n'existe donc **pas** de catégorie « composant d'autorisation non lié à la sécurité ». Deux
possibilités, une seule acceptable :

- OASIS **ne peut pas** empêcher un arrêt → il reste hors du périmètre SRP/CS ;
- OASIS **peut** l'empêcher → il entre dans le périmètre, au PLr applicable.

**Aujourd'hui OASIS est dans le second cas**, et c'est un défaut : `actuation_decision` applique
ses 7 conditions à tout ordre. Un ordre d'arrêt dont le `cmd_seq` est en retard, dont le
`boot_id` ne correspond pas, ou qui arrive pendant que l'état des capteurs est défavorable, est
**refusé**. Sur ce point précis, OASIS est contraire à 1.2.1 al. 4 f) (« l'arrêt […] n'est pas
empêché »).

### Ce que la partie G ne fait pas

**L'arrêt d'urgence ne passe pas par OASIS.** ISO 13850:2015 4.1.1.3 : « The emergency stop
function is a **complementary protective measure** and **shall not be applied as a substitute for
safeguarding measures** ». L'arrêt d'urgence est un circuit local, dédié, qu'OASIS ne peut pas
atteindre — c'est l'architecture à écrire dans la documentation de l'intégrateur.

Ce que la partie G spécifie est un **ordre d'arrêt réseau** : un moyen supplémentaire, non
sûr au sens des normes, qui doit malgré tout se comporter de façon asymétrique pour ne pas
dégrader ce qui existe.

---

## 1. Partie G — ordre d'arrêt asymétrique

### G.1 Classe d'ordre

Nouveau type d'ordre `OAC1` : un champ `class` à deux valeurs, **signé** comme le reste de la
charge utile (donc couvert par la signature v0B).

| Classe | Sens | Conditions appliquées |
|---|---|---|
| `Act` (existant) | agir : écrire un registre, bouger | **les 7 existantes + 1 nouvelle** (supervision, partie H) |
| `Stop` (nouveau) | aller dans le sens sûr | **3 seulement** (voir G.2) |

⚠️ Compatibilité : les ordres existants n'ont pas de champ `class`. L'octet est pris dans les
**4 octets réservés** que `parse_oac1` exige déjà nuls depuis le correctif de fuzzing de la
phase 2.3 — donc un ordre d'une version antérieure décode en `class = Act`, et les 15 tests
`act_*` existants restent valides sans modification.

### G.2 La règle d'arrêt

```
stop_decision(i) =
    Reject(NotVerified)   si !v0b_ok
    Reject(NotAuthorized) si !stop_authorized
    Reject(Revoked)       si revoked
    Stop                  sinon
```

**Trois conditions, et aucune autre.** Ne s'appliquent **pas** à un arrêt : `fresh`
(`boot_id`/échéance), `r14_safe`, `within_limits`, `cmd_seq`.

Justification, condition par condition :

| Condition | Appliquée ? | Pourquoi |
|---|:--:|---|
| `v0b_ok` | **oui** | Sinon n'importe quel attaquant extérieur arrête la flotte. Un arrêt non authentifié est un déni de service gratuit |
| `stop_authorized` | **oui** | Nouvelle permission `STOP`, distincte d'`ACTUATE`. Un nœud peut avoir le droit d'arrêter sans avoir celui d'agir — c'est le cas d'usage du superviseur |
| `revoked` | **oui** | Un nœud révoqué est précisément celui contre lequel on se défend ; lui laisser l'arrêt lui donne un déni de service permanent. ⚠️ **C'est le seul endroit où une condition de sécurité peut bloquer un arrêt**, et c'est un choix assumé : il est documenté et il est borné (il faut une révocation signée par l'opérateur) |
| `fresh` (`boot_id`, échéance) | **non** | Un arrêt périmé reste un arrêt. Rejouer un arrêt sur une machine déjà arrêtée est sans effet |
| `r14_safe` | **non** | **Inversion du sens** : un état de capteurs défavorable est une raison d'arrêter, pas de refuser l'arrêt |
| `within_limits` | **non** | Un arrêt n'a pas d'amplitude à borner |
| `cmd_seq` | **non** | Voir `fresh`. Le dédoublonnage Bloom de v0B écarte déjà les rejeux exacts au niveau mesh |

### G.3 Verrou d'arrêt

ISO 13850:2015 4.1.1.2 : « it **shall be maintained until it is manually reset** » et « it shall
**not be possible for any start command to be effective** ».

Transposition :

- un `Stop` accepté positionne un **verrou** `stopped: bool` dans l'actionneur ;
- tant que le verrou est posé, **tout `Act` est refusé** avec une nouvelle raison
  `Reason::Stopped` ;
- le verrou **ne se lève pas par un ordre réseau**. Il se lève par une **action locale
  explicite** (sur le banc : la commande `@E` sur le port USB de la carte ; en production : une
  entrée physique de l'intégrateur).

**Conséquence assumée :** un seul arrêt réseau authentique immobilise la machine jusqu'à
intervention humaine. C'est le sens sûr, et c'est aussi un vecteur de déni de service pour une
origine habilitée. Ce compromis est **documenté**, pas découvert par le client — et c'est
précisément pourquoi la permission `STOP` est distincte d'`ACTUATE`.

### G.4 État sûr sur perte de liaison

Dernière phrase de 1.2.1 : « Pour la commande sans fil, une défaillance de la communication ou de
la connexion ou une connexion défectueuse **n'entraîne pas de situation dangereuse**. »

C'est la partie H qui l'implémente : la perte de liaison est détectée comme une **absence de
balise de supervision**, et elle retire l'autorisation d'agir sans poser le verrou d'arrêt (une
liaison qui revient doit pouvoir reprendre sans intervention humaine, contrairement à un arrêt
explicite).

### G.5 Invariants à prouver (Kani)

1. `proof_stop_never_blocked` — **l'invariant central.** Pour **toute** entrée, si `v0b_ok &&
   stop_authorized && !revoked`, alors `stop_decision(i) == Stop`. Aucune valeur de `now_ms`,
   `deadline_ms`, `cmd_boot_id`, `r14_safe`, `within_limits`, `cmd_seq`, `last_executed_seq` ne
   peut changer ce résultat.
2. `proof_stop_requires_authenticity` — si `!v0b_ok`, le résultat n'est jamais `Stop`.
3. `proof_act_refused_while_stopped` — si le verrou est posé, `actuation_decision` ne rend
   jamais `Act`.
4. `proof_act_rule_unchanged` — pour `class = Act` et verrou levé et supervision vivante, la
   décision est **identique** à celle de l'actuelle `actuation_decision`. *Garantit la
   non-régression de la règle déjà prouvée et déjà validée sur silicium.*

### G.6 Tests

- `stop_*` : un arrêt passe avec `cmd_seq` en retard ; avec un `boot_id` d'un démarrage
  antérieur ; avec `r14_safe == false` ; hors échéance ; rejoué deux fois.
- un arrêt non signé est refusé ; un arrêt d'une origine sans permission `STOP` est refusé ; un
  arrêt d'une origine révoquée est refusé.
- après un arrêt : un `Act` valide est refusé (`Stopped`) ; après `@E` : le même `Act` passe.
- les **15 tests `act_*` existants restent inchangés et verts**.

---

## 2. Partie H — vivacité de la supervision

### H.1 Message

Nouveau message d'autorité **`OSB1`** (*OASIS Supervision Beacon*), kind 4 dans l'enveloppe
`OAU1` existante — donc il hérite gratuitement de l'octet de suite signé, de la politique de
suite minimale par type et du refus de déclassement de la phase 1.1.

Charge utile : `supervisor_fp ‖ actuator_boot_id ‖ beacon_seq ‖ validity_ms`.

Il est **lié au `boot_id` de l'actionneur** comme un ordre, pour la même raison : une balise
d'un démarrage antérieur ne doit pas prolonger l'autorisation après un redémarrage.

### H.2 Règle

L'actionneur maintient `supervision_until_ms`. À la réception d'une `OSB1` valide (signature,
origine habilitée `SUPERVISE`, non révoquée, `actuator_boot_id` correspondant, `beacon_seq`
strictement croissant) :

```
supervision_until_ms = min(now_ms + validity_ms, now_ms + MAX_SUPERVISION_MS)
```

Huitième condition de la porte, **appliquée aux `Act` seulement** :

```
si now_ms > supervision_until_ms  ->  Reject(SupervisionLost)
```

Position dans l'ordre des conditions : **après `fresh`, avant `r14_safe`**. Raison : c'est une
condition de contexte, au même titre que la fraîcheur, et la placer avant `r14_safe` garde les
conditions coûteuses à la fin.

`MAX_SUPERVISION_MS` borne ce qu'une balise peut accorder, pour qu'une balise rejouée avec un
`validity_ms` énorme ne puisse pas accorder une autorisation illimitée — même si `beacon_seq`
l'en empêche déjà, la borne est une seconde barrière.

### H.3 Ce que cela coûte en messages radio

À documenter avec les chiffres de [`compliance/PQC.md`](compliance/PQC.md) : une balise doit
arriver au moins une fois par `validity_ms`. À SF12, le budget légal est de **12 messages/heure**
pour une trame de 64 octets — donc `validity_ms ≥ 300 000` (5 min) est le plancher réaliste en
LoRa longue portée, et une supervision à la seconde est **impossible** sur ce lien. C'est une
limite à écrire, pas à cacher : la supervision fine suppose un lien à plus fort débit.

### H.4 Invariants à prouver (Kani)

1. `proof_act_needs_live_supervision` — si `now_ms > supervision_until_ms`, `Act` est impossible.
2. `proof_supervision_bounded` — `supervision_until_ms - now_ms <= MAX_SUPERVISION_MS` après
   toute balise acceptée, pour tout `validity_ms` y compris proche de `u64::MAX` (pas de
   débordement).
3. `proof_stop_ignores_supervision` — une supervision morte ne bloque **jamais** un arrêt.

### H.5 Tests

`sup_*` : balise acceptée puis `Act` passe ; après expiration, `Act` refusé
(`SupervisionLost`) ; balise d'un `boot_id` antérieur refusée ; `beacon_seq` rejoué refusé ;
`validity_ms` extrême borné ; balise d'une origine sans permission `SUPERVISE` refusée ;
**arrêt accepté alors que la supervision est morte**.

---

## 3. Partie I — journal infalsifiable des décisions

### I.1 Entrée

Une entrée par décision de la porte, acceptée **ou refusée**, longueur fixe (32 octets) :

```
seq        u32   compteur d'entrées, strictement croissant, persisté
boot_id    u64   démarrage de l'actionneur
origin_fp  [u8;8] origine de l'ordre
cmd_seq    u32   compteur de l'ordre
class      u8    Act | Stop
decision   u8    Act | Stop | Reject(raison)
flags      u8    bits : r14_safe, within_limits, supervision_live, stopped
reserved   u8    nul, contrôlé au parsing (leçon du fuzzing de la phase 2.3)
```

### I.2 Chaîne de hachages

```
h_0 = SHA-256("OASIS-JOURNAL-v1" ‖ boot_id)
h_n = SHA-256("OASIS-JOURNAL-v1" ‖ h_{n-1} ‖ entrée_n)
```

Détecte : une entrée **modifiée** (le hachage ne suit plus), une entrée **supprimée** (rupture de
chaîne **et** trou dans `seq`), une entrée **insérée**.

### I.3 Persistance

- un anneau de secteurs flash dédiés, distincts des emplacements de `tx_lease` et du plancher de
  micrologiciel ;
- l'entrée est écrite **puis** `h_n` et `seq` sont mis à jour dans un enregistrement de tête à
  deux emplacements alternés, comme `DualSlotStore` — **le mécanisme déjà prouvé en coupure de
  courant** en phase 1.3 ;
- ordre d'écriture : entrée, puis tête. Une coupure entre les deux laisse une entrée orpheline,
  détectable et **non comptée** : le vérificateur signale « 1 entrée non confirmée », ce qui est
  honnête et non ambigu.

### I.4 Lecture et vérification

- `@G<seq>` sur l'USB rend les entrées depuis `seq`, en hexadécimal, avec la tête ;
- un outil PC `oasis_journal_verify` recalcule la chaîne et rend un code de sortie : 0 intacte,
  1 rupture, 2 trou de `seq`, 3 entrée non confirmée.

### I.5 L'ancrage — la partie qui rend la revendication honnête

Une chaîne de hachages ne prouve rien si l'attaquant peut réécrire la tête. **Sur RP2040, un
accès physique (BOOTSEL, SWD) réécrit tout.** Donc :

- **Revendication autorisée** : le journal est **infalsifiable face à un attaquant distant** et
  détecte la **corruption accidentelle**.
- **Revendication interdite** : « infalsifiable » tout court, ou « résiste à un accès physique ».

Pour aller plus loin sans élément sécurisé, l'ancrage externe : la tête `(seq, h_n)` est **signée
par le nœud** et diffusée périodiquement dans une balise `OJA1` (kind 5). L'opérateur conserve
les ancres. Une suppression devient alors détectable **même avec un accès physique**, dès lors
que l'opérateur détient une ancre antérieure — parce qu'il faudrait forger la signature du nœud
sur une tête cohérente avec un journal tronqué.

⚠️ L'ancrage est spécifié ici mais **implémenté après** G, H et le journal local : c'est la partie
qui coûte du budget radio (voir H.3).

### I.6 Correspondance réglementaire explicite

À écrire dans le code et dans le document d'acheteur :

| Exigence | Ce que l'entrée fournit |
|---|---|
| Annexe III **1.1.9 al. 5** — « recueillent la preuve d'une intervention légitime **ou illégitime** » | une entrée par décision, **acceptée comme refusée** — c'est le mot « illégitime » qui impose de journaliser les refus |
| Annexe III **1.2.1 al. 2 f)** — journal activé **5 ans** | ⚠️ **non couvert** par cette partie : la durée porte sur les interventions et les versions de logiciel téléchargées. À traiter avec la partie micrologiciel |
| CRA annexe **I-I-2 l)** — enregistrer et surveiller les activités internes, **avec possibilité de désactivation par l'utilisateur** | ⚠️ la désactivation **n'est pas** prévue : à discuter. Désactivable, un journal ne sert plus de preuve pour 1.1.9 |
| IEC 62443-4-2 **CR 2.11** — horodatages | ⚠️ **non couvert** : OASIS n'a pas d'horloge. Les entrées portent `boot_id` + `now_ms`, pas une date. À déclarer comme non applicable avec justification, ou à résoudre par l'ancrage signé côté opérateur, qui lui a une horloge |
| IEC 62443-4-2 **CR 2.9** — capacité de stockage et alerte de seuil | l'anneau écrase les plus anciennes ; un compteur d'écrasement est publié dans la tête |

**Trois exigences restent ouvertes après la partie I** (durée 5 ans, désactivation, horodatage).
Elles sont listées ici pour ne pas être découvertes plus tard.

### I.7 Invariants à prouver (Kani)

1. `proof_journal_append_is_chained` — pour toute entrée, `h_n` dépend de `h_{n-1}` **et** du
   contenu complet de l'entrée (une modification d'un seul bit change `h_n`).
2. `proof_journal_seq_monotone` — `seq` croît strictement d'une entrée à la suivante, sans
   débordement non géré.
3. `proof_journal_parse_total` — le parseur d'entrée est total sur toute entrée de 32 octets et
   refuse tout octet réservé non nul.
4. `proof_every_decision_is_logged` — **le vrai invariant utile** : la fonction qui enchaîne
   « décider puis journaliser » n'a aucun chemin qui rende une décision sans produire une entrée.

### I.8 Tests

`jrn_*` : chaîne vérifiée sur 100 entrées ; un bit modifié dans l'entrée 37 → rupture détectée à
l'entrée 37 ; entrée 50 supprimée → rupture **et** trou de `seq` ; entrée insérée → rupture ;
anneau qui boucle ; vérificateur PC sur un journal réel sorti d'une carte.

---

## 4. Plan de validation silicium

3 cartes RP2040, montage de la phase 1.4 réutilisé (A = équipement Modbus `rmodbus`, B = origine,
C = passerelle).

| # | Test | Attendu |
|---|---|---|
| **S1** | arrêt signé de B avec `cmd_seq` en retard, `boot_id` antérieur, et pendant `r14_safe == false` | `Stop` dans les trois cas ; **0 écriture** sur le bus de A ensuite |
| **S2** | `Act` valide après l'arrêt | `Reject(Stopped)`, 0 octet sur le bus |
| **S3** | `@E` local puis le même `Act` | `Act`, 1 écriture |
| **S4** | arrêt non signé, puis arrêt d'une origine sans `STOP`, puis arrêt d'une origine révoquée | refusés tous les trois |
| **S5** | supervision expirée puis `Act` | `Reject(SupervisionLost)` ; **et un arrêt passe quand même** |
| **S6** | balise rejouée, balise d'un `boot_id` antérieur | refusées |
| **S7** | journal : 20 décisions mêlant acceptations et refus, sorties par `@G`, vérifiées par `oasis_journal_verify` | code 0 |
| **S8** | **coupure de courant réelle** de C pendant l'écriture du journal | au redémarrage : chaîne intacte, ou exactement 1 entrée non confirmée signalée ; `seq` ne recule pas |
| **S9** | modification d'un octet du journal via `@X`/écriture flash, puis vérification | rupture détectée. ⚠️ **Non localisée** : une chaîne à tête unique ne peut pas désigner l'entrée modifiée (tous les hachages suivants diffèrent) — l'outil rapporte l'indice où la vérification s'arrête. Une **suppression**, elle, est localisée par le trou de `seq` |
| **S10** | contrôle de non-régression : la campagne v0B A→B→C et la campagne Modbus de la phase 1.4 **rejouées** | résultats identiques |

⚠️ **Je m'arrêterai pour vous demander avant la coupure de courant du test S8.**

## 5. Ce que cette phase ne ferme pas

- **C14** (stockage sécurisé, clé lisible en flash) : ne se ferme pas en logiciel.
- **C6** : fermé par [`KEY_LIFECYCLE.md`](KEY_LIFECYCLE.md), qui nomme ses cinq manques.
- **C3** : mesuré en partie J ; la réduction `OAS1` est spécifiée, **pas encore écrite**, et
  le respect du duty-cycle n'est **pas** appliqué par le code (§J.6).
- **C4** (émission d'un ordre sur le terrain) : section K, à venir.
- L'arrêt d'urgence **reste hors d'OASIS**, par conception.
- Les trois exigences ouvertes de I.6 (durée 5 ans, désactivation, horodatage).
- Aucune de ces parties ne fait d'OASIS une fonction de sûreté.

---

# Partie J — budget radio (C3)

**Écrit le 2026-10-08, avant tout code.** Ferme la partie *mesure* de
`POSITIONING_GAPS.md` **C3** et propose une réduction.

## J.1 Tailles réelles, relevées dans les logs silicium

Aucune estimation : ces longueurs sont celles que les cartes ont imprimées.

| Message | Charge utile | **Sur le fil** | Relevé dans |
|---|---:|---:|---|
| en-tête v0B | — | **99** | `mesh.rs:159` |
| en-tête v0C (pré-filtre) | — | **123** | `prefilter.rs` (99 + 8 + 16) |
| arrêt compact `OAS1` **proposé** | 11 | **110** | §J.4 |
| balise de supervision `OSB1` | 32 | **131** | `kind=OSB1,len=131` |
| ordre Modbus `OMB1` | 33 | **132** | `kind=OMB1,len=132` |
| ordre d'actionnement `OAC1` | 54 | **153** | `kind=OAC1,len=153` |
| attestation `OAU1` Ed25519 | 122 | **221** | `kind=OAU1,len=221` |
| révocation `ORV1`, 1 nœud | 135 | **234** | `evidence/silicon/2026-10-06/ef/` |
| révocation `ORV1`, 2 nœuds | 143 | **242** | idem (**+8 par nœud**) |
| révocation `ORV1`, 16 nœuds | 255 | **354** | *extrapolé* à +8/nœud, `MAX_REVOKED = 16` |

## J.2 Temps d'antenne et débit permis

Formule LoRa standard, BW 125 kHz, 8 symboles de préambule, en-tête explicite, CRC actif,
CR 4/5, optimisation bas débit active à SF11 et SF12 :

```
T_sym      = 2^SF / BW
T_preamble = (8 + 4,25) × T_sym
n_payload  = 8 + max(ceil((8·PL − 4·SF + 28 + 16·CRC − 20·IH) / (4·(SF − 2·DE))) × (CR+4), 0)
ToA        = T_preamble + n_payload × T_sym
```

Duty-cycle : ETSI EN 300 220-2 clause 4.4.3.2, `Tobs` = 1 h, **par bande** et non par canal —
les trois canaux LoRaWAN par défaut sont dans la bande M et **partagent** le budget :

```
Ton_cum_max = 1 % × 3 600 s = 36 s par heure et par bande
msgs/h      = floor(36 / ToA)
```

⚠️ **La formule de temps d'antenne est « non vérifiée à la source »** : la datasheet Semtech
SX1276 est derrière un portail commercial
([`compliance/PQC.md`](compliance/PQC.md)). Elle est **recoupée** : elle reproduit au
dixième de milliseconde la valeur publiée dans ce document (PL = 64 à SF12 → 2 793,5 ms),
et le script `lora_budget.py` échoue si ce n'est pas le cas.

### LoRa brut (plafond PHY 255 octets) — **[calcul]**

| Message | o | SF7 | SF8 | SF9 | SF10 | SF11 | SF12 |
|---|---:|---:|---:|---:|---:|---:|---:|
| arrêt compact `OAS1` | 110 | 0,18 s · 195/h | 0,33 s · 109/h | 0,59 s · 60/h | 1,11 s · 32/h | 2,38 s · 15/h | **4,27 s · 8/h** |
| balise `OSB1` | 131 | 0,22 s · 167/h | 0,39 s · 92/h | 0,70 s · 51/h | 1,27 s · 28/h | 2,79 s · 12/h | 5,09 s · 7/h |
| ordre `OMB1` | 132 | 0,22 s · 163/h | 0,39 s · 92/h | 0,70 s · 51/h | 1,27 s · 28/h | 2,79 s · 12/h | 5,09 s · 7/h |
| ordre `OAC1` | 153 | 0,25 s · 143/h | 0,44 s · 81/h | 0,80 s · 45/h | 1,44 s · 25/h | 3,12 s · 11/h | **5,74 s · 6/h** |
| attestation `OAU1` | 221 | 0,35 s · 103/h | 0,61 s · 58/h | 1,11 s · 32/h | 2,01 s · 17/h | 4,43 s · 8/h | 8,04 s · 4/h |
| révocation 1 nœud | 234 | 0,37 s · 97/h | 0,65 s · 55/h | 1,17 s · 30/h | 2,09 s · 17/h | 4,59 s · 7/h | 8,36 s · 4/h |
| révocation 16 nœuds | 354 | **fragmenter** | fragmenter | fragmenter | fragmenter | fragmenter | fragmenter |

### LoRaWAN (plafond par data rate, RP002-1.0.3) — le résultat qui compte

Plafond de charge utile applicative : **SF7/SF8 = 242 o, SF9 = 115 o, SF10/SF11/SF12 = 51 o**.

> **L'en-tête v0B fait 99 octets à lui seul. À SF10, SF11 et SF12, le plafond LoRaWAN est de
> 51 octets : aucun message OASIS ne passe, même vide.** Et à SF9 (115 o) seul l'arrêt
> compact passerait.

C'est la conclusion la plus utile de toute la partie J, et elle n'est pas une limite d'OASIS
mais un **choix de transport** : le plafond de 51 octets vient des tables régionales
LoRaWAN, pas du modem. En **LoRa brut**, le PHY accepte 255 octets à tous les facteurs
d'étalement, et `oasis-lora-transport` annonce déjà `max_payload() = 255`.

**Décision à écrire dans la documentation d'intégration : OASIS ne se déploie pas sur
LoRaWAN classe A au-delà de SF9. Le transport visé est LoRa brut.** Sinon, chaque message —
y compris un simple ordre — exige la fragmentation, et la fragmentation d'un ordre de 153
octets en 3 trames de 51 à SF12 coûte ≈ 7,2 s du budget horaire, soit **5 ordres par heure**.

## J.3 Trois constats

1. **Un ordre, à SF12 et en LoRa brut, coûte 5,74 s : 6 ordres par heure et par bande.**
   C'est l'enveloppe opérationnelle réelle de la longue portée, et elle n'est pas négociable.
2. **Une révocation de 16 nœuds (354 o) dépasse 255 octets** et doit être fragmentée. La
   fragmentation existe et est prouvée sur silicium (`OFR1`, phase 1.1), donc la promesse
   « flotte entière révoquée » tient — mais elle coûte 2 trames, soit ≈ 9 s à SF12, donc
   **3 révocations par heure**. `POSITIONING_GAPS.md` C3 disait que la promesse « ne tient
   pas » : elle tient, mais à ce prix, et c'est une correction à porter.
3. **La supervision fine est impossible à longue portée.** 12 balises/heure à SF11, 7 à
   SF12 : le plancher de `MAX_SUPERVISION_MS` (5 min, partie H) est donc le bon ordre de
   grandeur, et il a été choisi avant ce calcul — ce qui se vérifie ici plutôt que de se
   supposer.

## J.4 La réduction à implémenter : l'arrêt compact `OAS1`

La partie G a établi qu'un arrêt n'est évalué que par **3 conditions** : trame v0B valide,
permission `STOP`, origine non révoquée. Aucune des autres n'est lue.

Donc un arrêt n'a pas besoin de transporter ce que personne n'évalue. `OAC1` traîne 54
octets dont **43 inutiles pour un arrêt** : `boot_id` (8), `deadline_ms` (8), `force`,
`torque`, `velocity` et `pos[3]` (24), plus 3 octets réservés.

```
OAS1 = "OAS1" (4) | actuator_id u16 (2) | cmd_seq u32 (4) | reserved u8 = 0 (1)   = 11 o
```

- `actuator_id` : **0 = tous les actionneurs du nœud**, `n` = celui-là seulement. Un arrêt
  de flotte et un arrêt ciblé, sans format supplémentaire.
- `cmd_seq` : **non évalué par la règle**, conservé pour le journal — sans lui, deux arrêts
  sont indistinguables dans la trace, et la partie I exige une preuve exploitable.
- l'octet réservé doit être nul : un arrêt a **exactement un encodage** (la leçon du
  défaut `parse_oac1` trouvé par fuzzing, puis du trou de 4 octets trouvé par Kani).

**Gain : 153 → 110 octets, −28 %.** À SF12 : 5,74 s → 4,27 s, soit **6 → 8 arrêts par
heure (+33 %)**. Un arrêt est le message qu'on veut pouvoir répéter quand la liaison est
mauvaise ; c'est donc le bon endroit où gagner.

### Ce que cela ne change pas

`OAC1` **classe `Stop` reste accepté** : `OAS1` est un encodage de plus, pas un
remplacement. Aucune garantie déjà prouvée sur silicium n'est retirée, et les 19 tests de
la campagne du 2026-10-08 restent valides tels quels.

### Invariants à prouver (Kani)

1. `proof_oas1_parse_total` — le parseur est total ; un `OAS1` parsé se ré-encode vers les
   mêmes 11 octets ; l'octet réservé non nul est refusé.
2. `proof_oas1_and_oac1_stop_decide_identically` — **l'invariant qui justifie la
   réduction** : pour toute entrée, la décision prise sur un `OAS1` est **identique** à
   celle prise sur un `OAC1` de classe `Stop` portant le même `actuator_id` et le même
   `cmd_seq`, quelles que soient les valeurs des 43 octets abandonnés.
3. `proof_oas1_never_acts` — un `OAS1` ne peut **jamais** produire une décision `Act`, quel
   que soit son contenu : il n'y a pas de chemin de l'arrêt vers l'action.

### Tests

`oas1_*` : aller-retour ; octet réservé non nul refusé ; `actuator_id = 0` arrête les deux
actionneurs du nœud (LED **et** passerelle Modbus — la leçon du 2026-10-08) ; `actuator_id`
ciblé n'arrête que celui-là ; équivalence avec `OAC1` classe `Stop` sur un échantillon de
valeurs abandonnées ; **les 26 tests des parties G/H/I restent inchangés**.

### Plan silicium

| # | Test | Attendu |
|---|---|---|
| J1 | `OAS1` avec `actuator_id = 0` depuis B | `Stop`, **les deux** verrous posés, 110 octets sur le fil (vérifié dans `PAYLOAD_TX`) |
| J2 | ordre valide puis ordre Modbus après J1 | `Reject(Stopped)` les deux, **0 écriture** sur le bus de A |
| J3 | `@Zc`, puis le même `OAS1` avec `actuator_id = 1` | `Stop`, LED verrouillée, **passerelle libre** (ordre Modbus accepté) |
| J4 | `OAS1` d'une origine sans `STOP` | `Reject(NotAuthorized)`, aucun verrou |
| J5 | journal après J1–J4, vérifié au PC | `intact`, les classes `Stop` visibles avec leur `cmd_seq` |
| J6 | non-régression : `OAC1` classe `Stop` toujours accepté | `Stop` |

## J.5 Deux réductions analysées et **écartées pour l'instant**

Les écrire ici évite de les redécouvrir.

**Révocation par différence.** Envoyer seulement les empreintes ajoutées depuis l'époque
précédente ramènerait 234 → ≈ 103 octets pour un nœud ajouté. **Écartée** : la liste est un
sur-ensemble *par construction*, propriété sur laquelle reposent `revocation_transition` et
3 preuves Kani. Un delta exige en plus une règle de détection de trou (époque sautée) et un
chemin de rattrapage obligatoire, sinon un nœud ayant manqué une époque garde une liste
**incomplète** — c'est-à-dire exactement la défaillance que la révocation existe pour
empêcher. Changer un protocole prouvé pour 130 octets n'est pas le bon échange.

**Index de signataire au lieu de la clé publique.** `ORV1` transporte les 32 octets de la
clé publique du signataire, que le nœud **détient déjà** (`owner_ed`). Un index d'un octet
économiserait 31 octets par signataire, et retirerait au passage un champ qu'un récepteur
doit de toute façon vérifier comme cohérent. **Reportée, pas rejetée** : c'est la réduction
suivante la plus propre, mais elle touche le format d'autorité partagé par la révocation,
l'enrôlement, la propriété et les manifestes de micrologiciel — donc quatre chemins prouvés
sur silicium, pour 31 octets. À faire avec sa propre campagne, pas en passant.

## J.6 Ce que la partie J ne fait pas

- **Aucune radio.** Tout le temps d'antenne est **calculé**, et la formule est « non vérifiée
  à la source ». Il n'y a toujours pas de LoRa sur ces cartes : le lien est un UART filaire.
  Le premier essai radio réel devra confirmer ces chiffres, pas les supposer.
- **Aucun respect du duty-cycle dans le code.** Le budget est documenté, il n'est pas
  appliqué : rien n'empêche le firmware d'émettre plus de 36 s par heure. C'est une
  obligation réglementaire (ETSI EN 300 220-2, décision 2019/1345/UE) et elle reste
  **ouverte** — à traiter avec le transport radio, pas avant.
- **Aucune réduction de l'en-tête v0B.** Les 64 octets de signature Ed25519 sont
  irréductibles sans changer de primitive, et les 35 autres portent le réseau, l'origine, le
  compteur et le condensé de charge utile — tous liés par la signature. Il n'y a rien à
  gagner là sans perdre une propriété prouvée.
