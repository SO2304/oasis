# Choix du segment de tête — comparaison chiffrée (phase 1, 2026-10-07)

Ferme `POSITIONING_GAPS.md` **D2** (« trop de segments »). Les quatre segments de
`POSITIONING.md` §3 sont ramenés à **trois candidats** : les robots agricoles sont une
variante du segment 1 (même règlement, même acheteur, terrain plus hostile) et les
« infrastructures de terrain » une variante du segment 2.

> ⚠️ **Aucune donnée de marché chiffrée dans ce document.** Taille de marché, prix de
> référence, nombre de prospects : **non recherchés en phase 0**, donc absents. Les chiffres
> ci-dessous sont des **dates réglementaires** (sources primaires dans
> [`docs/compliance/`](../docs/compliance/)), des **preuves déjà au dépôt** (logs silicium) et
> des **estimations de charge de travail explicitement marquées**. Les estimations de durée
> valent pour une personne seule au rythme observé dans ce dépôt (une phase mesurée et
> documentée ≈ une à deux journées de travail dense).

---

## 1. Tableau de décision

| | **S1 — Machines mobiles autonomes** | **S2 — Actionneurs du parc existant** | **S3 — Flottes de drones PX4** |
|---|---|---|---|
| **Déclencheur d'achat daté** | **20 janvier 2027** : annexe III 1.1.9 et 1.2.1 du règlement (UE) 2023/1230 deviennent opposables. Sanctions notifiées par les États **le 20 octobre 2026** | **18 octobre 2024** : NIS2 déjà applicable. **11 décembre 2027** : CRA pleinement applicable au fournisseur. Pression continue, pas d'échéance unique | **Aucun déclencheur réglementaire en Europe.** L'EASA a **retiré** les exigences de cybersécurité de SORA 2.5 (ED Decision 2025/018/R). Seul (UE) 2019/945 impose aux classes **C2 et C3** une liaison « protégée contre les accès non autorisés », sans moyen prescrit |
| **Acheteur réel** | Le **fabricant de la machine** (art. 8 et 10 : tout pèse sur lui). Fonction visée : responsable conformité produit / ingénieur système, pas le RSSI | L'**exploitant** (entité importante NIS2, annexe II point 5 d), NACE division 28) ou son **intégrateur OT**. Fonction visée : responsable OT / RSSI industriel | L'**intégrateur ou OEM drone**. Fonction visée : architecte embarqué. Acheteur technique, pas conformité |
| **Pourquoi il achète** | Il doit **produire la preuve** à l'autorité, y compris « le code source ou la logique de programmation, sur demande motivée » (art. 10(3)). Il n'a pas de chaîne descendante : il doit l'obtenir **par contrat** | Un ordre sur une vanne n'est pas authentifié. Référence douloureuse : **au moins 75 automates Unitronics** compromis par mots de passe par défaut (alerte CISA, 2023). NIS2 art. 21(2) d) et 21(3) le forcent à instruire ses fournisseurs directs | Parce que la signature MAVLink 2 a **9 limites documentées** par PX4 et ArduPilot eux-mêmes, et qu'il le sait déjà. Argument technique pur |
| **Normes et textes à satisfaire** | **4** : règlement (UE) 2023/1230 annexe III 1.1.9 + 1.2.1 + partie 3 ; ISO 3691-4:2023 (AMR) ; ISO 13849-1:2023 (par renvoi, cybersécurité **hors périmètre**) ; ISO 13850:2015 (arrêt d'urgence, référence normative d'ISO 3691-4) | **3** : IEC 62443-4-2 (SL 2 en capacité) ; NIS2 art. 21 ; CRA annexe I dès monétisation | **2** : (UE) 2019/945 annexe, parties 3 et 4 ; CRA annexe I dès monétisation. SORA : rien d'exigible |
| **Forme de produit attendue** | **Fournisseur de preuve** + bibliothèque Rust + firmware de référence. ⚠️ **Pas** un « composant de sécurité » au sens de l'art. 3(3) : OASIS n'assure pas une fonction de sécurité, et le prétendre serait faux | **Passerelle matérielle** placée devant l'équipement existant — c'est la seule forme qui protège un automate qu'on ne peut pas modifier | **Bibliothèque Rust** + couche MAVLink. Le plus simple à livrer : pas de matériel, pas de certification |
| **Ce qui est déjà prouvé et réutilisable** | Porte d'actionnement 7 conditions (silicium), v0B (0/150 inversions), révocation ORV1 avec coupure de courant, enrôlement et transfert de propriété, mise à jour A/B signée, pré-filtre DoS mesuré | **Tout S1, plus la phase 1.4 entière** : équipement `rmodbus` indépendant sur A, **4 décisions « agir » = 4 trames = 4 écritures** octet pour octet, 12 refus + 1 malformé + 3 injections brutes = **0 octet** sur le bus | **Tout S1, plus PX4 SITL déjà fonctionnel** au dépôt : mission 4 points, tenue d'altitude 1,94 m contre 2,0 m visés |
| **Manques bloquants à fermer** | **3** : **C5** (journal infalsifiable — exigé 4 fois par les textes), **C2** (arrêt asymétrique + état sûr sur perte de liaison — exigé par 1.2.1), **C12** (vivacité de la supervision — « si la fonction de supervision n'est pas active, la machine ne peut pas fonctionner ») | **2** : **C5** (IEC 62443-4-2 CR 2.8 et sa grappe de 6), **C14** (stockage sécurisé — ne se ferme **pas** en logiciel). Plus, techniquement : RS-485 au lieu de TTL, 1:N au lieu d'un équipement, carte de registres **signée** au lieu de compilée | **1** : **B1** (transporter un ordre OASIS dans un message MAVLink, vérifié par la porte avant exécution sur PX4 SITL) |
| **Distance à un pilote** *(estimation)* | **La plus longue.** 3 phases de développement + **zéro matériel AMR au dépôt** : il faut un partenaire qui prête une machine. Le pilote de 6 semaines de `POSITIONING.md` §8 suppose son banc | **Moyenne.** 1 phase (C5) + un transceiver RS-485 (quelques euros) pour un pilote crédible. **C14 restera ouvert** et sera la première objection du RSSI | **La plus courte.** 1 phase, **sur du matériel que vous avez déjà** (SITL, pas de drone nécessaire pour la démonstration d'attaques) |
| **Objection la plus dure** | « Vous n'êtes pas certifié, et vous êtes **une personne seule**. » Le CRA impose **5 ans** de support et **10 ans** de disponibilité des mises à jour dès monétisation (D9). Et une certification IEC 62443 exige d'abord un **SDLA d'entreprise** (CSA-100 v4.3 §4.1) | « Donnez-moi un produit, pas une bibliothèque. » Et : « votre clé est lisible en flash par BOOTSEL » (C14) | « Je n'ai aucune obligation de le faire. » Rien ne force l'achat en Europe |
| **Risque de perdre 6 mois** | Faible sur la demande (la date est dans le JO), **élevé sur la forme** : si l'acheteur exige un composant certifié, la réponse est non | **Élevé** : le cycle de vente OT est long et passe par des intégrateurs | **Élevé** : sans déclencheur, l'achat dépend d'un seul champion technique |

---

## 2. Ce que les textes disent, en une ligne chacun

- **S1** — annexe III, partie 3 : « les machines ou les produits connexes commandés à distance
  sont conçus et construits de façon à **ne répondre qu'aux signaux des unités de commande
  prévues** ». C'est, à un mot près, la phrase de positionnement d'OASIS, dans un règlement
  applicable dans **15 mois**.
- **S2** — IEC 62443-4-2 **CR 7.1** (*Denial of service protection*) : c'est la première question
  d'un acheteur OT, et c'est la seule du corpus qui soit désormais **mesurée** chez nous —
  179,4 ms → 0,70 ms par trame forgée, 60/60 messages légitimes livrés sous flot.
- **S3** — (UE) 2019/945, partie 3 point 8 et partie 4 point 12 : « être équipé d'un système de
  liaison de commande et contrôle **protégé contre les accès non autorisés aux fonctions de
  commande et de contrôle** ». Aucune occurrence de « authentification », « signature » ou
  « chiffrement » dans tout le règlement : l'exigence est téléologique, donc satisfiable — mais
  **non vérifiable**, donc non différenciante.

## 3. Le piège commun aux trois, à décider une fois

Le Règlement Machines **n'organise aucune chaîne d'obligations descendante** vers un composant
intégré (art. 8 et 10 : tout pèse sur le fabricant). Le CRA dit la même chose dans l'autre sens :
un composant libre **non monétisé** est hors de son champ (considérants 18 et 20), mais l'article
13 §5 oblige l'intégrateur à une **diligence raisonnable** sur ce composant.

**Conclusion valable pour les trois segments :** le produit vendable n'est pas le code — il est
sous licence MIT. C'est le **dossier de preuve** que l'intégrateur doit constituer et qu'il ne
sait pas produire seul : campagnes d'attaque sur son banc, logs bruts, tableaux de correspondance
réglementaire, et l'accès au code que l'article 10(3) peut exiger. Ce dépôt en produit déjà
8 campagnes datées et vérifiées par condensé.

## 4. Ce que je recommande — et ce que je ne décide pas

**Ma recommandation reste S1**, pour une seule raison qui domine les autres : **c'est le seul
segment où une date de calendrier force la décision d'achat**, et cette date est à 15 mois. Les
trois manques à fermer (C5, C2, C12) sont tous **exigés nommément par le texte**, donc le travail
technique et le travail commercial sont le même travail.

**Mais la décision est la vôtre**, et voici l'argument honnête contre ma recommandation :

- **S3 donne une démonstration en une phase, sur du matériel déjà au dépôt.** Si l'objectif à
  court terme est de parler à quelqu'un plutôt que d'avoir raison sur le papier, S3 est le chemin
  le plus rapide vers une conversation technique crédible — et `POSITIONING_GAPS.md` **D1**
  (zéro entretien) est classé 🔴 depuis le début.
- **S2 a la preuve la plus spectaculaire déjà faite** : un équipement industriel réel, sur un
  code tiers indépendant, où seules les décisions « agir » ont mis un octet sur le bus. C'est la
  démonstration la plus facile à montrer à un acheteur, et elle est déjà filmée dans les logs.

Autrement dit : **S1 maximise la probabilité qu'on vous achète en 2027 ; S3 minimise le délai
avant le premier entretien ; S2 maximise l'effet de la démonstration que vous avez déjà.**

Les trois sont compatibles à terme — C5 sert les trois, et c'est le premier travail dans tous les
cas. Le choix porte sur **ce qui vient après C5**.
