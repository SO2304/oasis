# Rapport final — exécution de `prompts/POSITIONING_ALIGNMENT.md`

**Branche `positioning-alignment`, du 2026-10-07 au 2026-10-08.** Dernier commit au moment
d'écrire : `aecccd9`. `main` n'a pas été touché : il faut un *fast-forward* ou une *pull
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
| Fermé avec preuve | 1 | 0 | 6 | 0 | **7** |
| Fermé (documentaire) | 1 | 0 | 1 | 2 | **4** |
| Partiellement fermé | 5 | 3 | 4 | 1 | **13** |
| Ouvert | 4 | 3 | 3 | 5 | **15** |
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
branche, et **69 commits**. Modules nouveaux : `journal`, `quorum`, `mavlink_order`,
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

---

## 4. Ce que ma propre vérification a trouvé — et qui serait passé sans elle

C'est la partie du rapport qui vaut le plus, parce qu'elle dit ce que les preuves coûtent.

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
11. **Deux « FAILED » Kani lus d'abord comme des réfutations** alors que la vérification en
    échec était une *unwinding assertion* — CBMC disant qu'il n'a pas pu dérouler assez
    loin. Et **le script d'exécution étiquette à tort** tout `FAILED` comme
    « counterexample » : défaut noté dans `evidence/kani/2026-10-08/b1/run1/README.md`, et
    commun aux campagnes précédentes.

**Dans la méthode :**

12. **Un essai qui n'en était pas un.** K6 prétendait tester un rejeu de balise ; une
    seconde balise rafraîchissait simplement la vue. Il a fallu ajouter `@Zb<ms>` pour
    forcer une horloge qui n'avance pas.
13. **Un raccourci de labo refusé.** La partie K était bloquée : le câblage mesh est à sens
    unique, donc C ne pouvait pas atteindre B. J'ai rapporté **K à 1/6** et **refusé** de
    relayer la balise par le PC, parce que c'est exactement le raccourci que le manque C4
    dénonce. Le fil déplacé, K est passé 6/6.

---

## 5. Ce qui reste ouvert, et pourquoi

### Deux points en attente de vous

- **B1, la moitié PX4 SITL.** « L'ordre valide arme le drone » **n'est pas montré** : PX4
  n'est plus installé (`/root/PX4-Autopilot` n'est pas lisible, `sudo` demande un mot de
  passe). Le logiciel est prouvé (7 tests, 4 Kani 4/4) ; la démonstration attend un accès.
- **Le fil `B.GP1`** est toujours sur `C.GP0` depuis la partie K : la chaîne A→B→C à trois
  sauts est **indisponible**, donc aucune vérification de non-régression mesh n'est possible.

### Les manques qu'aucun code ne ferme

| Manque | Pourquoi il ne se ferme pas ici |
|---|---|
| **D1** validation client | Il faut des entretiens. `CUSTOMER_DISCOVERY.md` est prêt ; **zéro** entretien mené |
| **C14** stockage sécurisé | **Ne se ferme pas en logiciel.** Clé lisible par BOOTSEL ou SWD. Réponse honnête : il faut un autre silicium |
| **C11** radio réelle | **Aucune radio n'a jamais émis.** Le pilote SX1262 n'est testé que contre un mock, tous les budgets sont calculés |
| **C13** confidentialité mesh | Les ordres circulent en clair. Aucun travail fait |
| **D5, D9** une personne, 5 à 10 ans de support | Un engagement, pas une tâche technique |
| **C6** rotation de clé de nœud | Nommé dans `KEY_LIFECYCLE.md` §6. C'est le prérequis qui gouverne toute migration cryptographique |
| **C3** duty-cycle | Calculé et documenté, **non appliqué par le code**. Obligation réglementaire documentée et non tenue |
| **C10** suite Kani complète | 130/152 au seul passage intégral ; **182 harnais existent**. « 182/182 » serait faux |

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
