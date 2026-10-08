# Rapport final — exécution de `prompts/POSITIONING_ALIGNMENT.md`

**Branche `positioning-alignment`, du 2026-10-07 au 2026-10-08.** Dernier commit au moment d'écrire : `7f9558a`, **75 commits** devant `origin/main`, qui est désormais **ancêtre de HEAD** (un *fast-forward* est possible). `main` n'a pas été touché : il faut un *fast-forward* ou une *pull
request*.

> **OASIS n'est pas une fonction de sûreté certifiée** (ni PL au sens de l'ISO 13849-1, ni
> SIL au sens de l'IEC 62061) et ne réduit aucun risque machine.

---

## 1. Les cinq phases, et ce qu'elles ont produit

| Phase | Demandé | Produit | État |
|---|---|---|---|
| **0** | Vérifier à la source chaque ligne des sections A et B, un fichier par texte, ajouter les manques trouvés, **s'arrêter** sur une synthèse avec une recommandation de segment | [`docs/compliance/`](compliance/) : **8 fichiers**, verbatim + lien + date d'applicabilité + tableau exigence → couverture → preuve → manque, et une section « limites » listant chaque « non vérifié à la source ». **Section F** de `POSITIONING_GAPS.md` : **11 corrections** aux lignes existantes et **10 manques nouveaux** (A9–A12, B6–B7, C12–C14, D8–D9) | ✅ |
| **1** | Comparaison chiffrée de ≤ 3 segments, **« je choisis ; ne choisis pas à ma place »** | [`partners/SEGMENT_COMPARISON.md`](../partners/SEGMENT_COMPARISON.md), avec l'argument **contre** ma recommandation écrit en clair. Choix retenu : **S1, machines mobiles autonomes** | ✅ |
| **2** | Fermer les 🔴 bloquants ; **spec montrée avant de coder** | [`docs/AUTHORITY_HARDENING_SPEC.md`](AUTHORITY_HARDENING_SPEC.md), parties **G** (arrêt asymétrique), **H** (vivacité de la supervision), **I** (journal infalsifiable), **J** (budget radio + arrêt compact `OAS1`), **K** (`TimeView`). Plus [`docs/KEY_LIFECYCLE.md`](KEY_LIFECYCLE.md) (C6) et `docs/lora_budget.py` (C3) | ✅ |
| **3** | C8, C10, B1, C9, A6 | C8 vocabulaire (`5bed8f8`, 10 documents, 0 fichier Rust) ; C10 fuzzing + `cargo-audit`/`deny` + SBOM + `SECURITY.md` ; **C9** ordres à deux signatures ; **A6** [`docs/CRYPTO_MIGRATION.md`](CRYPTO_MIGRATION.md) ; **B1** [`docs/specs/MAVLINK_ORDER_SPEC.md`](specs/MAVLINK_ORDER_SPEC.md) | ⚠️ B1 à moitié |
| **4** | `CUSTOMER_DISCOVERY.md` pour le segment retenu | [`partners/CUSTOMER_DISCOVERY.md`](../partners/CUSTOMER_DISCOVERY.md) : 5 hypothèses falsifiables avec seuils, grille de 30 min qui ne montre rien avant la minute 25, 20 types d'organisations avec la fonction visée | ✅ |
| **5** | Mettre à jour le positionnement avec **uniquement** ce qui est prouvé, + « Correspondance réglementaire » | **Section G** de `POSITIONING_GAPS.md` (statut des 42 manques) ; `POSITIONING.md` §10 ; `oasis_tech_en.html` §9 ; `COMPETITIVE_ANALYSIS.md` §5 ; chiffres corrigés dans 9 documents | ✅ |

---

## 2. Statut des manques — le tableau est en section G

Le statut de **chacun des 42 manques**, avec le chemin de la preuve ou ce qui manque, est
en **[section G de `POSITIONING_GAPS.md`](../partners/POSITIONING_GAPS.md#g-statut-de-chaque-manque-phase-5-2026-10-08)**.
Il n'est pas recopié ici : une seule source de vérité.

| Statut | A | B | C | D | **Total** |
|---|---:|---:|---:|---:|---:|
| Fermé avec preuve | 1 | 1 | 7 | 0 | **9** |
| Fermé (documentaire) | 1 | 0 | 1 | 2 | **4** |
| Partiellement fermé | 5 | 2 | 3 | 2 | **12** |
| Ouvert | 4 | 3 | 3 | 4 | **14** |
| Sans objet / information | 1 | 1 | 0 | 1 | **3** |
| | **12** | **7** | **14** | **9** | **42** |

**Les 9 manques 🔴 d'origine** — A1, B1, C1, C2, C3, C5, C6, D1, D2 — **sont tous sortis de
l'état « ouvert » sauf un** : **D1**, l'absence de validation client, qui ne se ferme que
par des entretiens que je ne peux pas mener. La préparation est faite ; le manque est
entier.

---

## 3. Ce que le code a gagné

Mesuré au **point de branchement** (`git merge-base origin/main positioning-alignment` =
`950be62`) et aujourd'hui, pas estimé :

| | Au point de branchement | Aujourd'hui | Mesuré par |
|---|---:|---:|---|
| Tests `oasis-rt` (lib) | 552 | **623** | `cargo test -p oasis-rt --release --lib` |
| Tests espace de travail | 591 | **662** | `cargo test --workspace --release` |
| Harnais Kani | 149 | **182** | `grep -rE 'kani::proof' oasis-rt/src \| wc -l` |
| Modules déclarés | 40 | **43** | `grep -cE '^pub mod ' oasis-rt/src/lib.rs` |
| Fichiers `src/*.rs` | 43 | **46** | `ls oasis-rt/src/*.rs \| wc -l` |

Soit **+71 tests de bibliothèque, +71 tests d'espace de travail et +33 harnais** sur la
branche, sur **71 commits**. Modules nouveaux : `journal`, `quorum`, `mavlink_order`,
`actuation/{wire,supervision,timeview}`.

⚠️ Ma première version de ce tableau donnait « 534 / 573 / 152 / 36 » : des chiffres de
mémoire, tous faux. Ceux-ci viennent d'un `git worktree` sur le point de branchement.

**Campagnes de preuve, toutes avec contrôle négatif** et toutes vérifiées par `SHA256SUMS`
**dans un clone frais après le commit** :

| Campagne | Résultat | Chemin |
|---|---|---|
| Kani parties G/H/I | 14/14 ; **1 réfutation réelle** au premier passage | `evidence/kani/2026-10-07/hardening/` |
| Kani parties J/K | 7/7, sans stub | `evidence/kani/2026-10-08/jk/` |
| Kani C9 (+ J/K relancées) | 12/12 | `evidence/kani/2026-10-08/c9/` |
| Kani B1 | 4/4 ; 2 *unwinding assertions* au premier passage | `evidence/kani/2026-10-08/b1/` |
| Silicium G/H/I | **19/20**, dont une **coupure de courant réelle** | `evidence/silicon/2026-10-08/hardening/` |
| Silicium J et K | 6/6 et 6/6 | `evidence/silicon/2026-10-08/jk/` |
| Silicium non-régression après C9 et B1 | **5/5** sur le firmware flashé, pas sur HEAD | `evidence/silicon/2026-10-08/regress/` |
| **B1 sur PX4 SITL** | **5/5** — le valide arme, les quatre autres non | `evidence/silicon/2026-10-08/b1-sitl/` |

---

## 4. Ce que ma propre vérification a trouvé — et qui serait passé sans elle

C'est la partie du rapport qui vaut le plus, parce qu'elle dit ce que les preuves coûtent.
**Six des seize sont des fautes de comptabilité ou d'affirmation à moi** (points 6, 8, 9, 10bis, 11bis et 13), et **trois des seize sont des essais qui ne testaient pas ce qu'ils annonçaient** (points 12, 14 et le R5 de la campagne de non-régression) : c'est
un taux, pas un accident, et la règle qui en sort est écrite dans chaque correctif —
**compter depuis l'arbre, jamais additionner des écarts mémorisés**.

**Dans le code :**

1. **Un trou de 4 octets dans le format du journal.** `ENTRY_LEN = 32`, les champs
   s'arrêtaient à 27, et un seul octet de réserve était vérifié : donc **plus d'un encodage
   par entrée**. Trouvé par une **réfutation Kani**, pas par un test. Corrigé (`f123447`).
2. **Un verrou d'arrêt posé et jamais lu.** L'arrêt latché bloquait la LED mais **pas** la
   passerelle Modbus, parce que `gateway_decision` appelait la règle sans contexte. Un
   arrêt d'urgence laissait donc écrire sur le bus d'un équipement. Corrigé (`13a3b5c`)
   avec deux tests qui affirment `frame.is_none()`.
3. **`content_kind` ne connaissait pas `OSB1`.** La partie H ne faisait **rien** tout en
   journalisant « vérifié, transmis ». Deux listes parallèles, l'une mise à jour et pas
   l'autre. Corrigé (`a09781e`) avec un commentaire d'avertissement sur la fonction.
4. **Un `assert!(x as usize < CONST)` qui ne compile pas** sous Kani (le `usize <` est lu
   comme une généricité) et qui compile partout ailleurs parce que les harnais sont
   derrière `cfg(kani)` : **14 harnais revenus « indéterminés » pour une erreur de
   syntaxe**. D'où le garde-fou `RUSTFLAGS="--cfg kani" cargo check`.
5. **Une ligne de log avant la boucle principale** a mis une carte en boucle de démarrage
   jusqu'à BOOTSEL (`Io::log` attend l'USB, chien de garde à 8 s, trois fois). **Trois
   diagnostics faux** avant le bon.

**Dans mes propres mesures et mes propres documents :**

6. **Trois chiffres de budget radio faux** dans la première version de
   `CRYPTO_MIGRATION.md` : j'avais facturé le temps d'antenne d'une trame de 64 octets en
   comptant des fragments de 51. Trouvés par les assertions de `docs/lora_budget.py`, qui
   recalcule au passage la valeur publiée par `PQC.md` pour vérifier que les deux documents
   comptent de la même façon.
7. **Deux défauts dans mon calcul des `CRC_EXTRA` MAVLink**, trouvés parce que le contrôle
   était les dix valeurs déjà au dépôt : les champs après `<extensions/>` sont exclus, et
   `uint8_t_mavlink_version` se hache comme `uint8_t`. Le même contrôle a trouvé **une
   erreur à moi** : j'avais noté 22 comme `CRC_EXTRA` de `PARAM_VALUE`, alors que 22 est
   son identifiant de message.
8. **Le compte des conditions de la porte était faux dans neuf documents** : six disaient
   « 7 » (vrai avant les parties G et H) et **quatre disaient « 10 », erreur de moi, du
   jour même**. C'est **9**. Recompté depuis `actuation_decision_ctx`.
9. **Mon premier compte des statuts donnait 41 au lieu de 42**, avec trois cases fausses.
   Le tableau est désormais **calculé par script** depuis le document.
10. **Un message de commit faux** : `b2d8e0b` annonce « 6 harnais Kani » pour C9 ; il y en
    a **5**. Corrigé dans le README de la campagne, pas dans l'historique.
10bis. **Quatre erreurs de comptabilité dans `CLAUDE.md` lui-même**, trouvées en vérifiant
    son arithmétique contre l'arbre : la liste de harnais par module sommait à **115** (elle
    omettait les 67 harnais des anciens modules), la liste incrémentale à **179**, et les
    deux chaînes de comptes de tests à **621** et **624** contre **623** mesurés. Les chaînes
    d'addition sont **supprimées**, pas rapiécées : une chaîne que personne ne peut vérifier
    vaut moins que pas de chaîne. Et une ligne **sous-estimait** le dépôt — « 3 harnais Kani
    écrits mais NON EXÉCUTÉS » pour le pré-filtre, alors que le passage existe, 3/3 vérifiés
    avec contrôle négatif, horodaté et condensé depuis le 2026-10-07.
11. **Deux « FAILED » Kani lus d'abord comme des réfutations** alors que la vérification en
    échec était une *unwinding assertion* — CBMC disant qu'il n'a pas pu dérouler assez
    loin. Et **le script d'exécution étiquette à tort** tout `FAILED` comme
    « counterexample » : défaut noté dans `evidence/kani/2026-10-08/b1/run1/README.md`, et
    commun aux campagnes précédentes.
11bis. **Une affirmation fausse, trouvée en fusionnant `main`.** `main` avait **15
    commits** que la branche n'avait pas, dont trois **correctifs de preuve** : `a3e4111` a
    **réfuté deux harnais mesh** — `proof_ad_reset_count_predictable` (le disjoint
    `|| actual_resets == 0` admettait un écart > 1) et
    `proof_af_combined_pattern_no_dashboard_alert` (seuil non borné, `threshold * 1000`
    débordait) — et cinq autres expiraient à 600 s. Donc la phrase **« aucun
    contre-exemple dans la suite »**, que la branche portait dans `CLAUDE.md`,
    `POSITIONING.md`, `DIFFERENTIATORS.md` et la page partenaires, était **fausse**. Elle
    est remplacée partout par ce qui est vrai : la suite **a produit deux
    contre-exemples**, ils étaient réels, ils sont corrigés. Une suite qui n'a jamais
    rien réfuté est une suite que personne n'a exécutée. `main` a aussi corrigé
    **« preuves brutes publiques »** — le dépôt est privé.

12. **Un essai qui ne testait rien, pris pour un succès possible (B1 sur SITL).** Mon
    motif d'attente de PX4 acceptait n'importe quelle ligne `INFO [commander]`, qui
    apparaît au bout de 2 s : l'ordre arrivait pendant le transitoire de démarrage, PX4
    répondait `Preflight Fail: no heading reference` puis `Arming denied`, et **le refus
    venait de l'EKF de PX4, pas de la porte**. Mesuré ensuite : PX4 est armable à **5 s**
    (`Ready for takeoff`). Sans cette correction, le même passage aurait pu être présenté
    comme « la porte a refusé ». Troisième occurrence de la même classe après K6 et R5.
13. **Une explication fausse, avancée puis retirée (B1 sur SITL).** Le véhicule ne reçoit
    jamais le flux de PX4 (`px4_heartbeats=0`), donc l'armement est établi par le **journal
    de l'autopilote** et non par l'observation interne du test. J'ai attribué cela au
    drapeau `-f` de `mavlink start`, en le croyant « diffusion » ; lecture faite de
    `mavlink_main.cpp`, **`-f` met `_forwarding_on`**, c'est-à-dire le transfert entre
    instances. Lier la socket à 14550 n'a rien changé. La cause reste **inconnue**, et le
    rapport de campagne le dit au lieu d'avancer une seconde hypothèse.

**Dans la méthode :**

14. **Un essai qui n'en était pas un.** K6 prétendait tester un rejeu de balise ; une
    seconde balise rafraîchissait simplement la vue. Il a fallu ajouter `@Zb<ms>` pour
    forcer une horloge qui n'avance pas.
15. **Un raccourci de labo refusé.** La partie K était bloquée : le câblage mesh est à sens
    unique, donc C ne pouvait pas atteindre B. J'ai rapporté **K à 1/6** et **refusé** de
    relayer la balise par le PC, parce que c'est exactement le raccourci que le manque C4
    dénonce. Le fil déplacé, K est passé 6/6.

---

### Fusion de `main`, le 2026-10-08

`main` n'était pas en retard : il portait **15 commits** absents de la branche, dont les
trois correctifs de preuve du §4-11bis et le script **`oasis-rt/kani_shards.sh`**. Ce
script attribue chaque harnais à exactement un *shard* de CI, et son `--check` **passe
sur les 182** de cette branche : la suite complète est donc **exécutable en CI**, même
si elle ne l'est pas sur cette WSL de 3,3 Go. Elle n'a toujours **jamais** tourné d'un
seul tenant, ni en CI ni en local.

Six conflits, **tous résolus en fusionnant les deux côtés**, jamais en choisissant.
`main` apportait des faits que la branche n'avait pas : le dépôt est privé donc les
preuves ne sont **pas** publiques, 17 partenaires Veridify nommés, la taille du firmware
en flash, une ligne de déploiement passerelle Modbus, une couche d'architecture
« identité et cycle de vie », et le paragraphe de loyauté sur WalnutDSA. La branche
apportait les neuf conditions de la porte, les chiffres courants et la phase 5.

Après la fusion : **623 tests de bibliothèque, 662 d'espace de travail, 182 harnais**,
couverture des shards verte, et `origin/main` est **ancêtre de HEAD** — donc un
*fast-forward* est possible.

## 5. Ce qui reste ouvert, et pourquoi

### Les deux points qui étaient en attente, et ce qu'ils ont donné

- **B1, la moitié PX4 SITL : faite le 2026-10-08, 5/5**
  ([`evidence/silicon/2026-10-08/b1-sitl/`](../evidence/silicon/2026-10-08/b1-sitl/)).
  `/root/PX4-Autopilot` est resté inaccessible — `/root` est en `drwx------`, donc aucun
  `chmod` sur son contenu ne pouvait suffire, et ma première recette était inopérante. PX4 a
  été recloné dans `$HOME`, et la chaîne de compilation montée **sans un seul `sudo`** :
  venv + `get-pip.py`, puis `cmake` et `ninja` en roues pip et les 14 dépendances Python en
  roues pour Python 3.14. `java` et `ant` sont absents de la machine et se sont révélés
  **inutiles** : l'airframe `10040_sihsim_quadx` fait voler PX4 avec sa physique interne.
  L'ordre valide arme (`Armed by external command`) ; forgé, altéré, rejoué et révoqué
  n'arment pas.
- **Le fil `B.GP1` remis sur `A.GP0`.** La chaîne à trois sauts est **recâblée mais pas
  fonctionnelle** : le firmware de A (équipement Modbus) maintient `GP0` haut et ne s'en
  sert jamais. Une non-régression **5/5** a tourné sur B→C
  ([`…/2026-10-08/regress/`](../evidence/silicon/2026-10-08/regress/)) — sur le firmware
  **flashé**, pas sur HEAD, parce que B et C sont sous le chargeur A/B avec plancher = version
  courante et que relever ce plancher est irréversible pour ce qui n'est qu'un champ de log.

### Les manques qu'aucun code ne ferme

| Manque | Pourquoi il ne se ferme pas ici |
|---|---|
| **D1** validation client | Il faut des entretiens. `CUSTOMER_DISCOVERY.md` est prêt ; **zéro** entretien mené |
| **C14** stockage sécurisé | **Ne se ferme pas en logiciel.** Clé lisible par BOOTSEL ou SWD. Réponse honnête : il faut un autre silicium |
| **C11** radio réelle | **Aucune radio n'a jamais émis.** Le pilote SX1262 n'est testé que contre un mock, tous les budgets sont calculés |
| **C13** confidentialité mesh | Les ordres circulent en clair. Aucun travail fait |
| **D5** une seule personne | Un engagement, pas une tâche technique. **D9** est désormais écrit dans `SECURITY.md` : aucune période de support n'est due aujourd'hui (hors champ du CRA faute de monétisation), et ce qui changerait le jour de la monétisation est dit, avec la réponse « non, pas seul » |
| **C6** rotation de clé de nœud | Nommé dans `KEY_LIFECYCLE.md` §6. C'est le prérequis qui gouverne toute migration cryptographique |
| **C3** duty-cycle | **Fermé le 2026-10-08** : appliqué par le transport, qui refuse d'émettre hors budget. Restent la formule non vérifiée à la source et l'absence de radio |
| **C10** suite Kani complète | **175/182 vérifiés** le 2026-10-08 en balayage séquentiel, 0 réfuté ; les 7 lourds restent à la CI, donc « 182/182 » serait encore faux |

---

## 6. Les trois phrases qu'on ne peut toujours pas dire

1. **« Résistant au déni de service »** sans la suite : contre un **initié**, le seau à
   jetons est un plafond de calcul qui **affame le trafic légitime** (4 messages légitimes
   livrés sur 60). Ce n'est pas un mécanisme d'équité.
2. **« Journal infalsifiable »** sans la suite : **tamper-évident contre un attaquant
   distant seulement**. Qui tient la flash réécrit la chaîne.
3. **« Prouvé sur le terrain »** : aucune radio, aucune machine mobile, aucun drone réel.
   Tout tient sur **trois RP2040 reliés par des fils**.

---

## 7. Ce que je recommande comme prochaine étape

**Des entretiens, pas du code.** C'est le seul manque 🔴 encore ouvert, et c'est celui qui
décide si les douze autres valent l'effort. Deux hypothèses de
[`CUSTOMER_DISCOVERY.md`](../partners/CUSTOMER_DISCOVERY.md) portent une règle d'arrêt
explicite : si **H2** (ils n'ont pas de méthode de preuve) ou **H3** (ils veulent un
dossier, pas un composant certifié) est réfutée, **il ne faut pas continuer à développer
pour S1**.

Et dans cette liste de vingt types d'organisations, trois ne sont pas des clients et sont
les plus utiles : l'**organisme notifié**, l'**autorité de surveillance du marché** et le
**laboratoire d'essais**. Ils savent ce qui sera accepté comme preuve en janvier 2027. Un
entretien avec eux peut réfuter H2 ou H3 plus vite que dix avec des fabricants.
