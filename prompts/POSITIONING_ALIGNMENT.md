# Prompt : aligner OASIS sur les besoins du marché (positionnement « autorité de commande »)

À coller dans Claude Code à la racine du dépôt OASIS, sur le PC où les 3 RP2040 sont
branchés. Base : la branche `claude/eloquent-ptolemy-oojjn0`, qui contient v0B, la
réserve de compteurs, la révocation, la porte d'actionnement et le correctif NaN.

---

## Objectif

Le positionnement est défini dans `partners/POSITIONING.md` :

> OASIS est la couche d'autorité de commande des flottes de machines autonomes :
> aucune machine ne bouge sur un ordre forgé, modifié, périmé, révoqué ou
> dangereux, même si un nœud de la flotte a été capturé.

`partners/POSITIONING_GAPS.md` liste ce qui manque pour qu'un acheteur sérieux
(RSSI, ingénieur sûreté, intégrateur PX4, responsable conformité) l'accepte. Ta
mission : **vérifier ces manques à la source, en trouver d'autres, puis fermer
ceux qui peuvent l'être avec ce qui existe**, sans jamais affirmer plus que ce qui
est prouvé.

## Règles non négociables

1. **Sources primaires.** Pour chaque exigence réglementaire ou normative, cite le
   texte officiel : article ou clause, et lien. Un blog ou un cabinet de conseil ne
   suffit pas comme seule source. Si tu n'as pas accès au texte (norme IEC payante),
   écris « non vérifié à la source » et indique d'où vient l'information.
2. **Aucune donnée inventée.** Chaque chiffre vient d'un log, d'un test ou d'un
   calcul montré. Une estimation est marquée « estimation ».
3. **Pas de revendication de sûreté de fonctionnement.** OASIS n'est pas une
   fonction de sûreté certifiée (SIL, PL). Ne l'écris jamais, même indirectement.
4. **Défensif uniquement.** Tous les tests d'attaque visent les propres nœuds
   d'OASIS, sur tes cartes ou sur PC.
5. Ne modifie pas le comportement de v0B, de la révocation ni de la porte sans
   test qui le justifie. `cargo test --workspace --release` reste vert.
6. Traçabilité des preuves : arbre commité et propre, logs en LF, `SHA256SUMS`
   vérifié dans un clone frais après le commit.
7. Branche `positioning-alignment`, commits petits et atomiques.

## Phase 0 : recherche (avant tout code)

Vérifie et complète chaque ligne de `POSITIONING_GAPS.md`, sections A, B et C.
Produis `docs/compliance/` avec un fichier par texte :

| Fichier | Contenu attendu |
|---|---|
| `MACHINERY_REGULATION_2023_1230.md` | Texte exact de l'annexe III 1.1.9 et 1.2.1 ; date d'application ; **périmètre** (les drones et aéronefs sont-ils exclus ? les robots mobiles autonomes sont-ils inclus ?) ; tableau exigence → ce qu'OASIS couvre → preuve → ce qui manque |
| `CRA.md` | Annexe I (exigences essentielles), article 14 (signalement), obligations d'un composant intégré par un fabricant, cas d'un projet open source sous licence MIT et rôle de « steward » |
| `EN_18031.md` | Exigences applicables à un nœud radio (authentification, contrôle d'accès, mise à jour), avec ce qu'OASIS couvre |
| `IEC_62443_4_2.md` | Les CR et EDR pertinents (identification, journal, intégrité, déni de service, mise à jour, gestion des clés), niveau SL visé, couverture |
| `IEC_TS_63074.md` | Comment OASIS s'inscrit dans le lien entre 62443 et la sûreté des machines, **sans** revendiquer de fonction de sûreté |
| `MAVLINK_SIGNING_GAP.md` | Limites de la signature MAVLink 2 (spécification officielle, PX4, ArduPilot), point par point, avec la réponse OASIS et son état (prouvé, conçu, absent) |
| `PQC.md` | Feuille de route post-quantique de l'UE, tailles de signatures ML-DSA, SLH-DSA, LMS/XMSS face à une trame LoRa de 255 octets, options réalistes |

Ajoute aussi à `POSITIONING_GAPS.md` tout manque que tu trouves et que je n'ai pas
listé. Pense au moins à : sûreté des machines (ISO 13849, IEC 62061, ISO 3691-4 pour
les véhicules autonomes), robots (ISO 10218), drones (règlements (UE) 2019/945 et
2019/947, U-space), NIS2 pour les opérateurs, contrôle des exportations
(cryptographie et drones), assurance et responsabilité, exigences des intégrateurs
(PX4, ArduPilot, ROS 2).

**Arrête-toi et montre-moi la synthèse de la phase 0**, avec ta recommandation de
segment de tête, avant la phase 1.

## Phase 1 : décision du segment de tête (avec moi)

Présente une comparaison chiffrée et sourcée d'au plus trois segments : par exemple
flottes de drones PX4, machines mobiles autonomes et robots agricoles soumis au
Règlement Machines, actionneurs industriels distants. Pour chacun : déclencheur
d'achat daté, acheteur réel, normes à satisfaire, forme du produit attendue,
distance entre l'état actuel et un pilote. **Je choisis ; ne choisis pas à ma place.**

## Phase 2 : fermer les manques techniques bloquants (🔴)

Une spec courte d'abord (`docs/AUTHORITY_HARDENING_SPEC.md`), puis code, tests,
Kani et silicium. Montre-moi la spec avant de coder.

1. **C1, déni de service par vérification forcée.**
   - Mesure d'abord : combien de faux messages par seconde, passant tous les
     filtres bon marché, suffisent à saturer un relais RP2040 ? Mesure-le sur tes
     propres cartes.
   - Ajoute un pré-filtre peu coûteux avant Ed25519, par exemple un MAC à clé de
     réseau de type v9, et une limite de débit par origine. Justifie le choix.
   - Mesure à nouveau. Rapporte les deux chiffres.
2. **C2, arrêt d'urgence et perte de liaison.** Spécifie et implémente une
   politique asymétrique : un ordre d'**arrêt** va toujours dans le sens sûr et ne
   doit jamais être bloqué par une condition qui ne le concerne pas ; un ordre
   d'**action** exige les sept conditions. Définis l'état sûr en cas de perte de
   liaison ou de capteur. Ajoute une preuve Kani : aucune entrée ne peut empêcher
   un arrêt valide de passer. Valide-le sur la LED de C.
3. **C3, budget radio.** Calcule, pour chaque type de message (ordre, révocation,
   balise), la taille et le temps d'antenne de SF7 à SF12 en EU868, et le nombre de
   messages par heure permis par le duty-cycle de 1 %. Puis propose et implémente
   au moins une réduction : encodage compact, fragmentation signée, ou révocation
   par différence. Teste-la.
4. **C5, journal infalsifiable.** Un journal des ordres acceptés et refusés en
   chaîne de hachages, avec la raison, le compteur, l'origine et le `boot_id`,
   persisté en flash, lisible et vérifiable depuis le PC. Montre sur silicium
   qu'une entrée modifiée ou supprimée est détectée. Relie-le explicitement à
   l'annexe III 1.1.9 du Règlement Machines.
5. **C6, cycle de vie des clés.** Écris `docs/KEY_LIFECYCLE.md` : génération, injection
   en fabrication, enrôlement d'un nœud, rotation, révocation, stockage de la clé
   opérateur (HSM ou k-sur-n), perte et récupération. Implémente l'outil
   d'enrôlement minimal côté PC s'il n'existe pas.
6. **C4, émission d'un ordre sur le terrain.** Spécifie comment le commandant
   obtient l'horloge et le `boot_id` de l'actionneur (balise signée ou
   défi-réponse), avec son coût en messages radio. Implémente la variante choisie
   et teste-la sur silicium.

## Phase 3 : manques 🟠 à fermer avec peu d'effort

- **C8, vocabulaire.** Renomme dans la documentation destinée aux acheteurs « porte
  R14 / entropie » en « verrou d'état des capteurs », sans changer le code, et
  ajoute la mention « pas une fonction de sûreté certifiée ».
- **C10, assurance logicielle.** Fuzzing des parseurs (`cargo-fuzz` : v0B, ORV1,
  OAC1, MAVLink), `cargo-audit` et `cargo-deny`, un SBOM (CycloneDX), et un
  `SECURITY.md` conforme au CRA (contact, délais, divulgation coordonnée,
  durée de support). Lance la suite Kani complète si la machine le permet ;
  sinon, dis-le.
- **B1, couche MAVLink.** Sur PX4 SITL, déjà utilisé dans le dépôt : transporter un
  ordre OASIS signé dans un message MAVLink (par exemple un message dédié ou un
  bloc de données), vérifié côté véhicule par la porte avant exécution. Montre
  qu'un ordre forgé, rejoué ou révoqué n'arme pas le drone et que l'ordre valide
  l'arme.
- **C9, ordre à deux signatures.** Pour une classe d'ordre critique, exige k
  signatures sur n via `oasis-operator-key`. Tests et preuve Kani.
- **A6, agilité cryptographique.** Un identifiant d'algorithme dans l'enveloppe ou
  un plan de migration écrit, sans casser v0B.

## Phase 4 : préparer la validation client (ce que tu ne peux pas faire à ma place)

Produis `partners/CUSTOMER_DISCOVERY.md` pour le segment choisi :
- 5 hypothèses à tester, chacune falsifiable ;
- une grille d'entretien de 30 minutes, sans présenter OASIS avant la fin ;
- les signaux qui valideraient ou invalideraient chaque hypothèse ;
- une liste de 20 types d'organisations à contacter, avec la fonction visée.

## Phase 5 : mettre le positionnement à jour

Mets à jour `partners/POSITIONING.md`, `partners/POSITIONING_GAPS.md` (statut de
chaque manque), la page technique `partners/oasis_tech_en.html` et
`partners/COMPETITIVE_ANALYSIS.md` avec **uniquement** ce qui est prouvé. Ajoute
une section « Correspondance réglementaire » qui renvoie vers `docs/compliance/`.

## Livrables

- `docs/compliance/*.md` et la synthèse de la phase 0.
- Spec, code, tests, preuves Kani et preuves silicium des phases 2 et 3.
- `partners/CUSTOMER_DISCOVERY.md`.
- Documents de positionnement mis à jour.
- Un rapport final : chaque manque de `POSITIONING_GAPS.md` avec son statut
  (fermé avec preuve, partiellement fermé, ouvert) et la raison.

## Quand t'arrêter pour me demander

- Après la synthèse de la phase 0, et pour le choix du segment (phase 1).
- Après chaque spec, avant de coder.
- Avant chaque coupure de courant.
- Si une exigence réglementaire semble impossible à satisfaire avec un RP2040.
- Si un correctif de sécurité dégrade une garantie déjà prouvée.
