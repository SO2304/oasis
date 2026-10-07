# OASIS : positionnement (référence, 2026-10-06)

Ce document est la source de vérité pour la page technique, la page partenaires,
LinkedIn et les échanges avec les prospects. Toute affirmation renvoie à une
preuve du dépôt. Rien n'y est affirmé sans preuve.

## 1. En une phrase

**EN :** *OASIS is the command-authority layer for autonomous machine fleets: no
machine moves on a forged, modified, stale, revoked or unsafe order, even when a
node of the fleet has been captured.*

**FR :** OASIS est la couche d'autorité de commande des flottes de machines
autonomes : aucune machine ne bouge sur un ordre forgé, modifié, périmé, révoqué
ou dangereux, même si un nœud de la flotte a été capturé.

## 2. Ce qu'on ne prétend plus

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
| 5 | **Sûreté d'action** : même un ordre valide n'est pas exécuté si la machine n'est pas en état d'agir | Porte R14 (incertitude des capteurs), limites physiques, refus des valeurs NaN ou infinies | Silicium : capteur perdu → `R14Unsafe` ; 1000 N → `OutOfLimits` ; NaN → refusé. 3 preuves Kani de la porte |

Et sur le logiciel : Rust `no_std`, 591 tests, 149 harnais Kani (123 vérifiés en CI,
20 nouveaux vérifiés un par un, 6 non vérifiés et nommés), preuves brutes avec SHA-256
vérifiées dans un clone frais (611 fichiers). Tourne sur un Cortex-M0+ à 1 $ (RP2040),
firmware d'environ 136 Ko de flash, 265–288 Ko avec l'autorité hybride, l'enrôlement et
la mise à jour.

Depuis le 2026-10-07, aussi prouvé sur silicium (avec limites, voir
`OASIS_VS_VERIDIFY.md`) : messages d'autorité hybrides Ed25519 + ML-DSA-44 avec
anti-rétrogradation, enrôlement et transfert de propriété, mise à jour A/B signée avec
coupures de courant, passerelle Modbus RTU devant un équipement existant.

## 5. Deux façons de l'utiliser

| Mode | Quand | État |
|---|---|---|
| **Mesh OASIS fermé** (v0B + révocation + porte) | Les relais ne sont pas dignes de confiance : chaque saut doit filtrer | TRL 4 : démontré sur 3 cartes, liaison filaire |
| **Couche d'autorité par-dessus un transport existant** (MAVLink, série, Reticulum ou microReticulum) | Le transport est déjà en place et mature ; on ajoute l'autorité, la fraîcheur, la révocation et la porte de sûreté **à l'arrivée** | Conçu pour : l'enveloppe v0B est un bloc d'octets indépendant du transport. **Pas encore testé** sur MAVLink ni sur Reticulum |

Le second mode est la réponse à « pourquoi pas Reticulum ? » : **on ne le
remplace pas, on s'installe au-dessus.** Reticulum transporte ; OASIS décide si
la machine a le droit d'agir.

## 6. Ce qui nous distingue de microReticulum (lu dans son code, commit `40fa628`)

| | microReticulum | OASIS |
|---|---|---|
| Ses relais vérifient les données | Non (`Transport.cpp` l.1980-2000) | Oui, origine, contenu, fraîcheur, réseau |
| Anti-rejeu | Liste d'empreintes persistée, non signée | Compteur signé, vérifiable par chaque relais |
| Exclusion d'un nœud | Listes « blackhole » locales | Révocation signée par l'opérateur, propagée |
| Lien avec les actionneurs | Aucun | Porte à 7 conditions, prouvée par Kani |
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
