# Durcissement de l'autorité de commande — spécification (phase 2, segment S1)

**Écrit le 2026-10-07, avant tout code.** Segment retenu : **machines mobiles autonomes**
(`partners/SEGMENT_COMPARISON.md`). Cette spécification couvre les **trois manques bloquants**
de ce segment, dans l'ordre où ils doivent être construits :

| Partie | Manque | Exigé par | Statut |
|---|---|---|---|
| **G** | **C2** — arrêt asymétrique et état sûr sur perte de liaison | annexe III **1.2.1 al. 4 d) et f)** et dernière phrase ; ISO 13850:2015 4.1.1.2 / 4.1.1.4 | à construire |
| **H** | **C12** — vivacité de la supervision | annexe III **partie 3** : « si la fonction de supervision n'est pas active, la machine ne peut pas fonctionner » | à construire |
| **I** | **C5** — journal infalsifiable des décisions | annexe III **1.1.9 al. 5** et **1.2.1 al. 2 f)** ; CRA annexe **I-I-2 l)** ; IEC 62443-4-2 **CR 2.8, 2.9, 2.10, 2.11, 2.12, 3.9** | à construire |

C3 (budget radio), C4 (émission d'un ordre sur le terrain) et C6 (`KEY_LIFECYCLE.md`) restent
dans la phase 2 et feront l'objet de sections J, K, L **après** validation silicium de G, H et I.

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
- **C3** (budget radio), **C4** (émission d'un ordre sur le terrain), **C6**
  (`KEY_LIFECYCLE.md`) : sections J, K, L.
- L'arrêt d'urgence **reste hors d'OASIS**, par conception.
- Les trois exigences ouvertes de I.6 (durée 5 ans, désactivation, horodatage).
- Aucune de ces parties ne fait d'OASIS une fonction de sûreté.
