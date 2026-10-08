# OASIS : positionnement (référence, mis à jour le 2026-10-08)

Ce document est la source de vérité pour la page technique, la page partenaires,
LinkedIn et les échanges avec les prospects. Toute affirmation renvoie à une
preuve du dépôt. Rien n'y est affirmé sans preuve.

> **Mise à jour du 2026-10-08 (phase 5 de `prompts/POSITIONING_ALIGNMENT.md`).** Le
> segment de tête est **S1 — machines mobiles autonomes**
> ([`SEGMENT_COMPARISON.md`](SEGMENT_COMPARISON.md)). Le statut de chaque manque est en
> **section G** de [`POSITIONING_GAPS.md`](POSITIONING_GAPS.md) : **7 fermés avec preuve,
> 4 fermés sur document, 13 partiellement fermés, 15 ouverts**. En cas de désaccord entre
> ce document et la section G, **c'est la section G qui a raison**. La correspondance
> réglementaire est en §10.

## 1. En une phrase

**EN :** *OASIS is the command-authority layer for autonomous machine fleets: no
machine moves on a forged, modified, stale, revoked or unsafe order, even when a
node of the fleet has been captured.*

**FR :** OASIS est la couche d'autorité de commande des flottes de machines
autonomes : aucune machine ne bouge sur un ordre forgé, modifié, périmé, révoqué
ou dangereux, même si un nœud de la flotte a été capturé.

## 2. Ce qu'on ne prétend plus

> ### ⚠️ OASIS n'est pas une fonction de sûreté
>
> OASIS **n'est pas une fonction de sûreté certifiée** : ni PL au sens
> d'ISO 13849-1, ni SIL au sens d'IEC 62061, et il n'est couvert par aucune
> certification de sûreté de fonctionnement. Il ne réduit aucun risque machine.
>
> Ce qu'il fait : décider si un ordre reçu est **authentique, habilité, frais et dans
> les limites configurées**. La réduction du risque reste assurée par le système de
> commande relatif à la sécurité du fabricant, indépendamment d'OASIS — et l'**arrêt
> d'urgence reste un circuit dédié qu'OASIS ne peut pas atteindre** (ISO 13850:2015
> 4.1.1.3).
>
> ISO 13849-1:2023 exclut explicitement la cybersécurité de son périmètre tout en
> reconnaissant qu'elle peut affecter les fonctions de sûreté, et renvoie à
> IEC TS 63074 pour ce sujet. C'est cette brèche désignée qu'OASIS occupe : **sous** le
> système de commande relatif à la sécurité, jamais à sa place. Détail et citations :
> [`docs/compliance/IEC_TS_63074.md`](../docs/compliance/IEC_TS_63074.md).

- **Pas un mesh LoRa généraliste.** Reticulum (TRL 7) et microReticulum (TRL 6-7)
  ont la radio, le routage, l'écosystème et l'anonymat. On ne les affronte pas.
- **Pas « meilleur que » un système plus mature.** OASIS est à **TRL 4** : modèle
  de sécurité démontré en labo, sur vrai silicium, sur liaison filaire.
- **Pas rapide.** Une vérification Ed25519 coûte ≈ 178 ms par saut sur Cortex-M0+.
  C'est fait pour des ordres et des messages d'autorité, pas pour de la télémétrie.

## 3. Pour qui

Les équipes qui exploitent une **flotte fermée de machines autonomes, sous
l'autorité d'un opérateur**, où un faux ordre a une conséquence physique :

| Segment | Le risque qui coûte cher |
|---|---|
| Flottes de drones (PX4) | Un drone capturé qui donne des ordres aux autres, ou un ordre rejoué |
| Robots agricoles et de terrain | Un robot volé sur une parcelle, réintroduit dans la flotte |
| Actionneurs industriels distants (vannes, pompes, portails, barrières) | Un ordre d'ouverture forgé ou rejoué sur une installation isolée |
| Infrastructures de terrain | Un équipement compromis qu'on ne sait pas exclure rapidement |

Ce qu'ils ont en commun : un **opérateur** (donc une autorité, à l'inverse d'un
réseau ouvert et anonyme), des nœuds **exposés physiquement**, et des **actions
irréversibles**.

## 4. Les cinq garanties, avec leurs preuves

| # | Garantie | Comment | Preuve (TRL 4) |
|---|---|---|---|
| 1 | **Autorité** : seule une origine habilitée peut commander une machine donnée | Signature Ed25519 par nœud ; liste d'autorités de commande par actionneur | Silicium : ordre de B (non habilité) refusé `NotAuthorized`, broche à 0 (`evidence/silicon/2026-10-06/ef/`) |
| 2 | **Intégrité, vérifiée à chaque relais** : un ordre forgé ou modifié meurt au premier saut | Origine, contenu (SHA-256), compteur et réseau dans la signature v0B ; mode strict sans déclassement | Silicium : 0/150 inversions de bit acceptées, 150/150 tracées ; contenu remplacé rejeté ; v0A refusé en mode strict (`…/followup/`) |
| 3 | **Fraîcheur** : un ordre rejoué ou périmé n'est jamais exécuté, même après une coupure de courant | Compteur signé, fenêtre persistée en flash, réserve de compteurs côté émetteur ; échéance de 10 s liée au `boot_id` de l'actionneur | Silicium : rejeu à l'octet près refusé après coupure du relais ; émetteur redémarré accepté ; ordre d'un ancien `boot_id` refusé |
| 4 | **Révocation de toute la flotte** sans renouveler les autres clés | Liste signée par l'opérateur (k-sur-n possible), époque croissante, persistée avant application, appliquée par chaque relais | Silicium : origine révoquée rejetée au premier saut, y compris après coupure ; ancienne liste refusée `Rollback`. k-sur-n : PC seulement |
| 5 | **Verrou d'état** : même un ordre valide n'est pas exécuté si la machine n'est pas en état d'agir | **Verrou d'état des capteurs** (seuil sur l'incertitude mesurée), limites physiques, refus des valeurs NaN ou infinies. ⚠️ **Ce n'est pas une fonction de sûreté certifiée** | Silicium : capteur perdu → refus ; 1000 N → `OutOfLimits` ; NaN → refusé. 3 preuves Kani de la porte |

Une sixième garantie, ajoutée le 2026-10-07 : **6 — Disponibilité face à un extérieur**.
Un pré-filtre à clé de lien (dérivée des identités, jamais distribuée) refuse une trame
forgée en 0,70 ms au lieu de 179,4 ms ; sous un flot de 8 trames forgées/s, le relais
livre **60 messages légitimes sur 60** à 19 % de charge, contre 39/60 à 110 % sans lui
(`evidence/silicon/2026-10-07/prefilter/`). ⚠️ Contre un **initié** (nœud enrôlé, ou clé
lue dans la flash d'un RP2040), le budget par liaison borne le calcul du relais mais
**affame le trafic légitime** (4/60) : c'est un plafond de calcul, pas de l'équité.

Et sur le logiciel : Rust `no_std`, **662 tests**, **182 harnais Kani**. Sur les 149
d'avant cette branche, **123 ont été vérifiés en CI** le 2026-10-06 et 6 ne l'ont pas
été, nommés ; les 33 ajoutés ici sont vérifiés par campagne, chacune avec son contrôle
négatif. **175 des 182 ont été vérifiés d'un seul balayage séquentiel** le 2026-10-08, 0 réfuté,
0 indéterminé ; les **7 lourds** restent à la CI, donc « 182/182 » serait encore faux. Et la
suite **a produit deux contre-exemples réels**
sur des harnais mesh, corrigés le 2026-10-08 : ce n'est pas une suite « sans
contre-exemple », c'est une suite qui a servi. Plus 1,5 × 10⁹ exécutions de fuzzing pour
un seul défaut trouvé et corrigé, SBOM CycloneDX et **0 vulnérabilité connue** sur les
6 arbres de dépendances ; preuves brutes avec SHA-256 vérifiées dans un clone frais.
Tourne sur un Cortex-M0+ à 1 $ (RP2040), firmware d'environ **136 Ko de flash**, 265 à
288 Ko avec l'autorité hybride, l'enrôlement et la mise à jour.

Prouvé sur silicium, avec ses limites (voir [`OASIS_VS_VERIDIFY.md`](OASIS_VS_VERIDIFY.md)) :
messages d'autorité hybrides Ed25519 + ML-DSA-44 avec anti-rétrogradation, enrôlement et
transfert de propriété, mise à jour A/B signée avec coupures de courant réelles,
passerelle Modbus RTU devant un équipement existant, arrêt asymétrique qui se verrouille,
vivacité de la supervision, et un journal de décisions infalsifiable à distance.

## 5. Deux façons de l'utiliser

| Mode | Quand | État |
|---|---|---|
| **Mesh OASIS fermé** (v0B + révocation + porte) | Les relais ne sont pas dignes de confiance : chaque saut doit filtrer | TRL 4 : démontré sur 3 cartes, liaison filaire |
| **Couche d'autorité par-dessus un transport existant** (MAVLink, série, Reticulum ou microReticulum) | Le transport est déjà en place et mature ; on ajoute l'autorité, la fraîcheur, la révocation et la porte de sûreté **à l'arrivée** | **Fait pour MAVLink** : l'ordre voyage dans un `V2_EXTENSION` (msgid 248) et la porte décide avant l'armement — 7 tests sur un vrai routeur, 4 preuves Kani 4/4 (`evidence/kani/2026-10-08/b1/`). ⚠️ **Rien sur PX4 SITL** : « l'ordre valide arme le drone » n'est pas montré. Reticulum : toujours pas testé |

Le second mode est la réponse à « pourquoi pas Reticulum ? » : **on ne le
remplace pas, on s'installe au-dessus.** Reticulum transporte ; OASIS décide si
la machine a le droit d'agir. Depuis le 2026-10-08 ce mode n'est plus une intention :
il existe pour MAVLink, en logiciel ([`docs/specs/MAVLINK_ORDER_SPEC.md`](../docs/specs/MAVLINK_ORDER_SPEC.md)).

## 6. Ce qui nous distingue de microReticulum (lu dans son code, commit `40fa628`)

| | microReticulum | OASIS |
|---|---|---|
| Ses relais vérifient les données | Non (`Transport.cpp` l.1980-2000) | Oui, origine, contenu, fraîcheur, réseau |
| Anti-rejeu | Liste d'empreintes persistée, non signée | Compteur signé, vérifiable par chaque relais |
| Exclusion d'un nœud | Listes « blackhole » locales | Révocation signée par l'opérateur, propagée |
| Lien avec les actionneurs | Aucun | Porte à 9 conditions, prouvée par Kani, et un arrêt qui n'en garde que 3 |
| Maturité | **TRL 6-7**, radio réelle, écosystème | **TRL 4** |

## 7. Ce qu'on ne sait pas encore (à dire avant qu'on nous le demande)

- Pas de radio testée sur matériel ; pas de routage (inondation avec TTL).
- Pas de protection matérielle des clés sur RP2040 : un nœud capturé livre sa clé
  par SWD. Sa signature seule ne permet pas d'usurper les autres, et il peut être
  révoqué. Prévu : RP2350 (OTP, secure boot).
- Un nœud révoqué peut encore relayer, retarder ou jeter le trafic des autres
  (pas le modifier).
- Pas d'audit externe, pas de certification, consommation non mesurée.
- Aucune attaque n'a encore été exécutée contre un concurrent : les comparaisons
  viennent de lectures de code.
- **Le journal est tamper-évident contre un attaquant distant seulement** : qui tient
  la flash réécrit la chaîne (C14, et C14 ne se ferme pas en logiciel).
- **Le duty-cycle radio est appliqué par le code** depuis le 2026-10-08 : le transport
  LoRa possède son budget et refuse d'émettre hors duty cycle, avec le délai d'attente
  dans l'erreur. ⚠️ Mais la formule de time on air est **non vérifiée à la source**, et
  **aucune radio n'a jamais émis** : l'exactitude est celle de la formule, pas d'une mesure.
- **Aucune rotation de clé de nœud**, et `MAX_REVOKED = 16` est un plafond dur : c'est le
  prérequis qui gouverne le calendrier d'une migration cryptographique
  ([`docs/CRYPTO_MIGRATION.md`](../docs/CRYPTO_MIGRATION.md) §6-b).
- **Les ordres mesh circulent en clair** : v0B et v0C authentifient, ils ne chiffrent pas
  (C13).
- **Zéro entretien client mené** (D1). Les segments restent des hypothèses, et les cinq
  hypothèses de [`CUSTOMER_DISCOVERY.md`](CUSTOMER_DISCOVERY.md) sont toutes « non testée ».

## 8. L'offre

**Un pilote de 6 semaines** sur le matériel du partenaire, pour passer de TRL 4 à
TRL 5-6 ensemble :
1. Intégration de la porte d'autorité sur une classe d'actionneur réelle (un
   moteur, une vanne, une commande PX4).
2. Exercice de révocation : on « capture » un nœud, on le révoque, on mesure le
   temps d'exclusion de toute la flotte.
3. Démonstration d'attaques sur leur banc : ordre forgé, rejoué, périmé, d'une
   origine révoquée ; aucune ne doit faire bouger la machine.
4. Critères de réussite fixés **avant** le test ; logs bruts remis au partenaire.

## 9. Formulations prêtes à l'emploi

- **Titre LinkedIn :** *Building OASIS · No machine moves on a forged, replayed or revoked order · Command authority for autonomous fleets · Rust no_std · Looking for field pilots*
- **Accroche orale :** « Si un de vos drones ou robots est volé demain, combien de temps vous faut-il pour l'exclure de la flotte, et qu'est-ce qui l'empêche de donner des ordres aux autres ? »
- **Réponse à « pourquoi pas Reticulum ? » :** « Reticulum est un excellent transport. OASIS n'est pas un transport : c'est ce qui décide si une machine a le droit d'exécuter un ordre. Les deux se combinent. »

---

## 10. Correspondance réglementaire

Chaque texte a son fichier dans [`docs/compliance/`](../docs/compliance/), avec le
**verbatim**, le lien vers la source, la date d'applicabilité et un tableau exigence →
couverture OASIS → preuve → manque. Les normes payantes (IEC, ISO) portent la mention
**« non vérifié à la source »** et disent d'où vient l'information.

| Texte | Fichier | Ce qu'OASIS apporte, et jusqu'où |
|---|---|---|
| **Règlement Machines (UE) 2023/1230** — annexe III 1.1.9 et 1.2.1, applicable le **20 janvier 2027** | [`MACHINERY_REGULATION_2023_1230.md`](../docs/compliance/MACHINERY_REGULATION_2023_1230.md) | Journal des ordres acceptés **et refusés**, chaîné et persisté (partie I, silicium + coupure de courant) ; identité du firmware (phase 1.3). ⚠️ La conformité reste celle du **fabricant de la machine** (art. 8 et 10) : OASIS fournit la preuve, pas le marquage |
| **Cyber Resilience Act** — signalement dès le **11 septembre 2026**, application le **11 décembre 2027** | [`CRA.md`](../docs/compliance/CRA.md) | SBOM CycloneDX, `cargo-audit`/`cargo-deny` à 0 vulnérabilité connue, `SECURITY.md` avec contact et délais 24 h/72 h, mise à jour signée A/B. ⚠️ Mise à jour **USB, pas par le réseau** ; les 5 ans d'assistance sont un engagement contractuel |
| **RED, acte délégué** + **EN 18031-1/2/3** | [`EN_18031.md`](../docs/compliance/EN_18031.md) | Authentification des messages, mécanisme de mise à jour sécurisé. ⚠️ **Aucune évaluation par un laboratoire**, et OASIS **n'a aucune radio** : l'applicabilité se juge sur un produit qui émet |
| **IEC 62443-4-2** (composants) | [`IEC_62443_4_2.md`](../docs/compliance/IEC_62443_4_2.md) | CR 1.2 identité ; **CR 2.8 journal auditable** ; **CR 7.1 déni de service, mesuré** (0,70 ms contre 179,4 ms par trame forgée) ; EDR 3.10 mise à jour. ⚠️ Intégrité du démarrage partielle (pas de secure boot) ; **-4-1 ouvert** |
| **IEC TS 63074:2023** | [`IEC_TS_63074.md`](../docs/compliance/IEC_TS_63074.md) | **Le document de la frontière** : il sert à dire qu'OASIS **n'est pas** une fonction de sûreté, et que le PL ou le SIL reste dans le système de commande certifié (ISO 13849-1, IEC 62061). ⚠️ Norme payante, non vérifiée à la source |
| **Feuille de route post-quantique de l'UE** — cas à haut risque migrés **fin 2030** | [`PQC.md`](../docs/compliance/PQC.md) + [`CRYPTO_MIGRATION.md`](../docs/CRYPTO_MIGRATION.md) | Messages d'autorité déjà **hybrides Ed25519 + ML-DSA-44** (silicium, coupure de courant), ce qui couvre nommément les « mises à jour de micrologiciel quantum-safe » du jalon 2030 ; plan de migration écrit pour le reste. ⚠️ **Aucun ordre par saut post-quantique sur LoRa** : c'est le budget légal d'émission, pas une difficulté d'ingénierie |
| **Signature MAVLink 2** — 9 limites documentées | [`MAVLINK_SIGNING_GAP.md`](../docs/compliance/MAVLINK_SIGNING_GAP.md) | Ordre signé **par nœud** transporté dans un `V2_EXTENSION`, vérifié par la porte avant armement (B1). ⚠️ **Rien sur PX4 SITL** |

**Ce que cette section ne dit pas** : aucune de ces lignes n'est une déclaration de
conformité. OASIS n'est ni certifié, ni audité, ni évalué par un organisme notifié, et le
Règlement Machines **n'organise aucune chaîne d'obligations descendante** vers un composant
intégré — ce qui est précisément pourquoi le livrable vendable est le **dossier de preuve**
et non le code, qui est sous licence MIT.
