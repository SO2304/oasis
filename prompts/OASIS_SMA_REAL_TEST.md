# Prompt : premier test réel — la passerelle OASIS devant un onduleur SMA

À coller dans Claude Code (Opus) à la racine du dépôt OASIS, sur le PC relié au
réseau de l'onduleur. Base : la branche `max-tech-2026-10-09` (ou `main` si elle y a
été fusionnée). Lis d'abord `CLAUDE.md`, `docs/PILOT_INSTALL.md`,
`docs/specs/MODBUS_TCP_SPEC.md`, `docs/specs/MODBUS_GATEWAY_SPEC.md`,
`partners/POSITIONING_GAPS.md` (ligne B4) et `tools/pilot_campaign.sh`.

---

## Contexte

Jusqu'ici, partout où un document OASIS dit « automate », il faut lire
`oasis_test_plc`, un simulateur. L'audit du 2026-10-09 a nommé la première preuve
manquante : **une écriture acceptée et une écriture refusée devant un équipement réel
du commerce, avec le journal relu.**

L'équipement disponible est un **onduleur photovoltaïque SMA Sunny Boy « Smart
Connected »**, raccordé au réseau, chez l'utilisateur, sur son propre réseau local.
Il parle Modbus TCP nativement (profil SMA, unit ID 3 ; profil SunSpec, unit ID 126 ;
port 502 ; codes de fonction 0x03, 0x04, 0x06, 0x10 d'après les sources
secondaires). **Le serveur Modbus est désactivé par défaut.**

**Objectif : montrer, sur cet onduleur et pas sur un simulateur, que seule une
décision `Act` de la passerelle atteint l'équipement, que chaque attaque est refusée
avant lui, et que le journal relu le prouve.**

**Répartition :** l'utilisateur fait le câblage et branche ce qu'on lui demande.
**Tout le reste est à ta charge** : découverte réseau, activation du Modbus sur
l'onduleur, configuration, clés, campagne, mesures, preuves, documents, commits.

## Règles non négociables

1. **Un seul registre écrivable, sans danger.** La seule écriture autorisée est la
   **limitation de puissance active** (SunSpec modèle 123, `WMaxLimPct` et son
   activation `WMaxLim_Ena`, ou l'équivalent du profil SMA). **Avant toute écriture,
   lis l'adresse exacte dans la documentation SMA correspondant au modèle et au
   firmware de cet onduleur**, pas dans un forum. Si tu ne la trouves pas dans une
   source SMA, arrête-toi et dis-le : aucune écriture « pour voir ».
   - Plage de la carte des registres : **50 à 100 %**. Jamais 0 %. L'onduleur
     doit toujours produire.
   - **Jamais d'écriture en boucle** : un rapport d'utilisateur décrit un onduleur SMA
     qui refuse ses paramètres après des écritures répétées. Au plus **une écriture
     légitime par minute**, et **10 écritures légitimes au total** sur toute la
     campagne.
   - Aucune autre adresse. Aucun registre de réseau, de protection, de pays, de
     tension, de fréquence. Si un test demande « un registre hors carte », l'adresse
     doit être une adresse **en lecture seule** ou inexistante : l'écriture est
     refusée par la passerelle avant d'atteindre l'onduleur, c'est le but, mais le
     choix doit être sûr même si la passerelle avait un défaut.
2. **La porte ne change pas.** Aucune modification de `modbus_gateway`,
   `modbus_tcp`, `actuation`, ni des binaires `oasis_mbtcp_*` pour « faire passer »
   l'onduleur. Si l'onduleur refuse une trame que `rmodbus` acceptait, c'est une
   **découverte** à documenter, pas à contourner. Un correctif n'est permis que s'il
   est couvert par un test, une preuve Kani là où une preuve existait, et noté dans
   le rapport comme « défaut trouvé sur équipement réel ».
3. **Un seul chemin vers l'onduleur pendant la campagne.** L'onduleur n'est joignable
   que par l'hôte de la passerelle. Demande à l'utilisateur le câblage qui le
   garantit (section « Câblage ») et **vérifie-le** par un scan depuis l'hôte agent :
   le port 502 de l'onduleur doit y être injoignable.
4. **Rust uniquement**, aucun nouveau Python (règle du dépôt, `tools/hmi_client.py`
   déjà déclaré excepté). Pour parler Modbus en direct dans le groupe témoin, utilise
   un binaire du dépôt ou `rmodbus` dans un test, pas `mbpoll` ni `pymodbus`.
5. **Pas de chiffre isolé** : K=10, médiane ± demi-écart, pour la latence
   IHM → réponse à travers la passerelle **et** en accès direct témoin.
6. **Honnêteté du vocabulaire.** Écris « onduleur SMA », jamais « automate ». Ce n'est
   pas une machine au sens du Règlement (UE) 2023/1230 : cette campagne **ne
   démontre pas** le 1.1.9, elle démontre l'autorisation par écriture sur un
   équipement réel. Pas de « conforme », pas de « certifié », pas de « prouvé sur le
   terrain » (les trois phrases interdites de `POSITIONING_GAPS.md` restent interdites).
7. **Chaque limite est écrite** dans le rapport, dans `CLAUDE.md` et dans
   `POSITIONING_GAPS.md` B4.
8. Avant et après chaque commit : `cargo test --workspace --release`, clippy, fmt,
   le build MCU de `CLAUDE.md`, `oasis-rt/kani_shards.sh --check`,
   `tools/check_claims.sh --full`. Les logs vont dans `evidence/` avec `git add -f`
   (les `.log` sont ignorés par défaut). L'empreinte `tree=` du rapport est celle du
   **commit testé**, pas d'un commit intermédiaire.
9. Commits sur la branche de travail, avec les lignes d'attribution de la session,
   **aucun identifiant de modèle** dans le dépôt. Pas de fusion dans `main` sans
   demande explicite.
10. **Aucune donnée personnelle dans le dépôt** : pas de mot de passe installateur,
    pas de numéro de série complet, pas d'adresse IP publique, pas de clé privée.
    Masque le numéro de série (4 derniers caractères) dans les logs avant de les
    committer.

## Câblage — la seule chose demandée à l'utilisateur

Deux topologies acceptables. Choisis celle que le matériel permet, et dis-le
clairement à l'utilisateur, en une liste de branchements, avant de commencer.

- **T1, deux hôtes** (préférée, c'est aussi la phase A.6 du prompt max-tech) :
  PC 1 = agent OASIS (côté « opérateur »), PC 2 = passerelle OASIS. PC 2 a **deux
  interfaces** : l'une vers PC 1 (câble Ethernet direct), l'autre vers l'onduleur
  (câble Ethernet vers son port réseau, ou le WLAN de l'onduleur). PC 1 n'a aucune
  route vers l'onduleur.
- **T2, un hôte, deux interfaces** : un seul PC, agent et passerelle en deux
  processus, l'onduleur sur une interface dédiée (Ethernet) et le réseau domestique
  sur l'autre. Moins fort : l'IHM et l'onduleur partagent une machine. Si c'est T2,
  écris-le comme limite.

Dans les deux cas : l'onduleur reste raccordé au réseau électrique, en production.
On ne touche à rien d'électrique. L'utilisateur donne : le modèle exact et le
firmware (plaque et interface web), le mot de passe **installateur** de l'interface
web (oralement ou dans un fichier hors dépôt), et l'heure à laquelle il y a du
soleil pour que la limitation soit visible.

## Phase 0 — découverte et activation (aucune écriture)

1. Trouve l'onduleur sur le réseau (mDNS, `arp`, scan du port 502 et 80/443 sur le
   segment). Relève modèle, firmware, numéro de série masqué.
2. **Active le serveur Modbus TCP** par l'interface web de l'onduleur. Si un outil de
   navigateur est disponible dans la session, fais-le toi-même. Sinon, donne à
   l'utilisateur la **séquence exacte de clics** (menu, paramètre, valeur), c'est la
   seule exception à « il ne fait que le câblage ». Note le port et les unit ID
   affichés.
3. Lecture seule, depuis l'hôte passerelle : lis le modèle SunSpec commun (1) et le
   modèle 123, la puissance AC instantanée, l'état de l'onduleur. Enregistre
   10 lectures de puissance à 30 s d'intervalle : c'est la **ligne de base**.
4. Vérifie la règle 3 : depuis l'hôte agent, le port 502 de l'onduleur est
   injoignable.
5. Écris `evidence/real/<date>/sma/PLAN.md` : topologie retenue, adresses, unit ID,
   le registre d'écriture retenu **avec la référence SMA** (document, section,
   version), la plage 50–100 %, la liste des cas de la phase 2. **Attends l'accord de
   l'utilisateur sur ce plan avant la première écriture.**

## Phase 1 — configuration de la passerelle

Suis `docs/PILOT_INSTALL.md` : une graine par hôte, `peer_pk` (jamais `peer`),
`peer_addr` = onduleur, `unit` = 126 ou 3 selon le profil retenu, carte des registres
réduite au seul registre d'écriture avec 50–100 (et le registre d'activation si le
profil l'exige, avec sa seule valeur valide), `origin_register` pour l'identité de
l'agent, un opérateur de révocation. Une valeur 32 bits occupe deux registres : la
carte et la trame FC16 doivent le prévoir. Démarre les deux processus avec les
unités `deploy/*.service` ou à la main, journal activé.

## Phase 2 — la campagne

Reprends la logique de `tools/pilot_campaign.sh` et, pour chaque cas, note : trame
envoyée, décision de la passerelle, réponse de l'onduleur, **puissance AC avant et
après** (lue par la passerelle), entrée de journal.

Cas obligatoires :

1. **Écriture légitime** : limitation à 60 %. La puissance AC doit baisser dans la
   minute (si le soleil le permet ; sinon, note la valeur du registre relu).
2. **Retour** à 100 %. La puissance remonte.
3. **Rejeu** octet pour octet du cas 1 : refusé, aucune trame vers l'onduleur.
4. **Origine forgée** (clé non enrôlée) : refusée.
5. **Valeur modifiée** dans une enveloppe authentique : refusée.
6. **Hors plage** : 40 % → refusé, registre inchangé.
7. **Registre hors carte** (adresse en lecture seule) : refusé.
8. **Ordre expiré** et **ordre d'un démarrage précédent de la passerelle** (redémarre
   la passerelle entre les deux) : refusés.
9. **Modbus brut**, écriture 60 % envoyée directement au port de la passerelle par un
   tiers : refusé, rien vers l'onduleur.
10. **Révocation** `ORV1` de l'agent, puis une écriture légitime de cet agent :
    refusée ; journal `Change(Revocation)`.
11. **Lecture** à travers la passerelle (`OMQ1`) : la puissance AC est relue par
    l'agent sans contact direct.
12. **Témoin** : depuis l'hôte passerelle, en direct, une limitation à 60 % puis 100 %
    avec un binaire Rust, pour prouver que l'onduleur obéit bien à ce registre.

Compte des écritures légitimes sur l'onduleur : cas 1, 2 et 12 = **4 au total**,
loin du plafond de 10.

À la fin : journal relu `intact` ; puis une entrée réécrite (refus → acceptation)
donne `exit 1`.

## Phase 3 — mesures

K=10 : latence IHM → réponse à travers la passerelle pour une écriture légitime,
avec l'espacement d'une minute imposé (donc les 10 écritures de K=10 **ne doivent
pas** changer la valeur : utilise 100 % → 100 %, qui est une écriture légitime sans
effet). Comparaison avec l'accès direct témoin, même espacement. Latence de lecture
`OMQ1` K=10.

## Phase 4 — documents

1. `evidence/real/<date>/sma/REPORT.md` : topologie, modèle, firmware, chaque cas
   avec ses extraits de log, les puissances avant/après, le journal relu, les
   latences, `SHA256SUMS`, et une section **Limites** : équipement d'énergie et non
   machine, une seule commande sans danger, un seul registre, aucune E/S, profil
   Modbus de l'onduleur non audité par nous, défauts trouvés le cas échéant.
2. `CLAUDE.md` : une ligne de matrice « Real device: SMA inverter over Modbus TCP »
   avec ses limites ; la phrase « non testé sur automate du commerce » reste vraie
   et doit rester écrite ; recompte avec les commandes du fichier.
3. `partners/POSITIONING_GAPS.md` B4 : ajoute ce qui est désormais prouvé sur
   équipement réel et ce qui ne l'est toujours pas.
4. `docs/PILOT_INSTALL.md` : un encart « Onduleurs SMA » avec la procédure
   d'activation et le piège des écritures répétées.
5. Un **script de rediffusion** `tools/sma_campaign.sh` qui rejoue la campagne à
   l'identique, avec les mêmes plafonds d'écriture codés en dur.

## Livrables

- Les commits des phases 0 à 4, chacun vérifié selon la règle 8.
- Un rapport final, en français simple, qui répond à quatre questions :
  - combien d'écritures ont atteint l'onduleur, et combien de décisions `Act` il y a
    eu (les deux nombres doivent être égaux) ;
  - quelles attaques ont été refusées, et où (avant la porte, par la porte) ;
  - ce que le journal relu prouve, et contre qui ;
  - ce que cette campagne **ne prouve pas**, en une liste que l'utilisateur pourra
    lire telle quelle à un client.
