# Validation client — segment S1, machines mobiles autonomes

**Écrit le 2026-10-08** (phase 4). Segment retenu en phase 1 :
[`SEGMENT_COMPARISON.md`](SEGMENT_COMPARISON.md) **S1 — machines mobiles autonomes**.
Acheteur visé : le **fabricant de la machine**, pas l'exploitant, parce que les articles 8
et 10 du règlement (UE) 2023/1230 font peser tout sur lui.

> **OASIS n'est pas une fonction de sûreté certifiée** (ni PL au sens de l'ISO 13849-1,
> ni SIL au sens de l'IEC 62061) et ne réduit aucun risque machine. Ce document sert à
> **tester si quelqu'un a le problème**, pas à vendre.
> Voir [`../docs/compliance/IEC_TS_63074.md`](../docs/compliance/IEC_TS_63074.md).

> ⚠️ **Aucune donnée de marché ici.** Taille, prix, volumes : jamais recherchés. Ce
> document ne contient que des hypothèses à falsifier, une grille et une liste de
> **types** d'organisations — pas de noms d'entreprises, qui demanderaient une recherche
> non faite.

---

## 1. Ce que ce document cherche à savoir

Le dépôt a accumulé des preuves techniques sérieuses. Aucune de ces preuves ne dit que
**quelqu'un paierait**. Les cinq hypothèses ci-dessous sont les cinq façons dont
l'ensemble du positionnement S1 peut être **faux**. Elles sont formulées pour pouvoir
être **réfutées** par une quinzaine d'entretiens, pas confirmées.

Le piège à éviter est connu et nommé : un fabricant à qui on présente une démonstration
d'attaque dit « impressionnant » et n'achète pas. La grille du §3 **ne montre rien** avant
la minute 25.

---

## 2. Cinq hypothèses falsifiables

### H1 — L'échéance du 20 janvier 2027 est déjà dans leur plan produit

**Hypothèse :** chez un fabricant de machine mobile autonome, l'annexe III 1.1.9 et 1.2.1
du règlement (UE) 2023/1230 est **déjà** un élément de planning identifié, avec quelqu'un
qui en est responsable.

**Réfutée si** : sur 15 entretiens, **moins de 5** savent citer une date, ou la personne
en face renvoie systématiquement vers « le service juridique » sans savoir qui s'en
occupe. Alors le déclencheur daté — le seul argument qui distingue S1 de S2 et S3 — n'est
pas un déclencheur d'**achat**, seulement un déclencheur de **conformité documentaire**,
et le segment perd ce qui le justifie.

**Confirmée si** : ≥ 8 nomment une date ou un jalon interne, **et** au moins 3 ont déjà un
budget ou un fournisseur en vue.

**Signal faible mais fort s'il apparaît** : quelqu'un cite **l'article 10(3)** (l'autorité
peut exiger « le code source ou la logique de programmation, sur demande motivée ») sans
qu'on le mentionne. C'est le signe d'une organisation qui a vraiment lu le texte.

### H2 — Ils ne savent pas comment prouver « ne répondre qu'aux signaux des unités de commande prévues »

**Hypothèse :** le fabricant sait qu'il doit satisfaire la phrase de l'annexe III partie 3,
mais **n'a pas de méthode** pour en produire la preuve, et le sait.

**Réfutée si** : ≥ 8 décrivent une méthode qui leur suffit (un audit par un tiers, une
clause fournisseur, une norme qu'ils appliquent déjà), **ou** si plusieurs répondent que
leur liaison est propriétaire et que cela suffit à leur certificateur. Dans ce dernier cas
le problème est résolu socialement, pas techniquement, et un dossier de preuve ne vaut
rien.

**Confirmée si** : ≥ 8 décrivent une absence de méthode, et au moins 3 emploient
spontanément un mot comme « on verra avec l'organisme notifié ».

**Ce qui l'invaliderait le plus vite** : découvrir qu'une norme harmonisée couvre déjà la
clause. Au 2026-10-07, **aucune norme harmonisée n'est citée au JOUE pour 2023/1230**
([`../docs/compliance/MACHINERY_REGULATION_2023_1230.md`](../docs/compliance/MACHINERY_REGULATION_2023_1230.md)) —
mais cela peut changer avant 2027 et il faut le revérifier à chaque trimestre.

### H3 — La forme attendue est un dossier de preuve, pas un composant

**Hypothèse :** ce qu'ils achèteraient est la **campagne d'attaque sur leur banc et ses
logs**, pas une bibliothèque à intégrer.

**Réfutée si** : ≥ 8 demandent un composant certifié, ou répondent qu'ils n'achètent que
du matériel référencé par leur service achats. C'est l'objection la plus dure du segment
et elle est déjà écrite dans `SEGMENT_COMPARISON.md` : *« si l'acheteur exige un composant
certifié, la réponse est non »*. Si H3 est réfutée, **S1 est fermé pour une personne
seule**, et il faut basculer sur S2.

**Confirmée si** : ≥ 5 disent qu'ils paieraient une prestation de preuve, et au moins 2
décrivent un budget d'ingénierie (pas d'achat de composant) comme la ligne qui
conviendrait.

**Signal décisif** : la question « est-ce que votre nom apparaît dans mon dossier
technique ? ». Qui la pose achète une prestation, pas un produit.

### H4 — Le journal de décisions est le livrable qui parle

**Hypothèse :** parmi tout ce que le dépôt sait faire, ce qui déclenche l'intérêt est le
**journal infalsifiable des décisions d'actionnement** — pas la cryptographie, pas la
résistance au déni de service.

**Réfutée si** : à la question ouverte « qu'est-ce qui vous serait utile ? » posée avant
toute présentation, **moins de 5** mentionnent une trace, un journal, un audit ou une
imputabilité. Alors le choix de faire du journal (partie I) le cœur de S1 est le mien, pas
le leur.

**Confirmée si** : ≥ 8 mentionnent une trace spontanément, et au moins 3 racontent un
incident où ils n'ont pas su dire **pourquoi** la machine avait agi.

**Pourquoi cette hypothèse est la plus importante à tester tôt** : quatre textes exigent
une journalisation (`docs/compliance/`), ce qui ne dit pas que les fabricants la
**veulent**. Un besoin réglementaire n'est pas un besoin.

### H5 — Le prototype peut se faire sans leur machine

**Hypothèse :** un premier pilote utile peut se faire sur un **banc** (une carte, un
actionneur, un bus), sans accès à une machine mobile réelle.

**Réfutée si** : ≥ 8 répondent que rien ne se décide sans un essai sur la machine, ou que
le prêt d'une machine est impossible pour des raisons d'assurance. Alors le segment exige
un partenaire matériel **avant** toute preuve, ce qui rallonge la distance au pilote déjà
qualifiée de « la plus longue » — et il faut le savoir avant de construire quoi que ce
soit d'autre.

**Confirmée si** : ≥ 3 acceptent un banc comme preuve intermédiaire, ou proposent un accès
à une machine de développement.

**Le dépôt n'a aucun matériel AMR.** Cette hypothèse décide s'il faut en chercher.

---

## 3. Grille d'entretien — 30 minutes, OASIS n'apparaît pas avant la minute 25

**Règle absolue : aucune démonstration, aucun nom de produit, aucun chiffre de performance
avant la minute 25.** Le but des 25 premières minutes est d'entendre leur problème dans
leurs mots, et ces mots serviront à réécrire `POSITIONING.md`.

**Avant l'appel**, écrire la réponse qu'on s'attend à entendre pour chaque hypothèse. On
ne peut pas être surpris par ce qu'on n'a pas prédit.

### 0–3 min — cadrage

> « Je prépare un travail technique sur la commande à distance des machines autonomes et je
> cherche à comprendre comment ça se passe vraiment chez vous. Je ne vends rien
> aujourd'hui. Vingt-cinq minutes de vos réponses, et je vous montre ce que je fais à la
> fin si ça vous intéresse. »
>
> « Décrivez-moi votre rôle et la machine dont vous êtes le plus proche. »

### 3–10 min — la machine et sa commande (ouvert, **ne pas** orienter)

> 1. « Comment une commande arrive-t-elle à cette machine aujourd'hui ? Qui peut lui
>    donner un ordre ? »
> 2. « La dernière fois qu'elle a fait quelque chose d'inattendu — qu'est-ce qui s'est
>    passé, et comment avez-vous su pourquoi ? » *(→ H4, posée comme un récit, pas comme
>    une question sur l'audit)*
> 3. « Qu'est-ce qui l'arrête, et que se passe-t-il si la liaison tombe ? »
> 4. « Si demain quelqu'un vous disait qu'un ordre reçu n'était pas le vôtre, comment le
>    vérifieriez-vous ? »

**Ne pas** demander « avez-vous besoin d'authentification ». La réponse est oui et elle
n'apprend rien.

### 10–18 min — la conformité, sans souffler la date

> 5. « Quelles obligations réglementaires pèsent sur cette machine, et qui s'en occupe ? »
>    *(→ H1 ; si le règlement machines sort tout seul, laisser parler ; **ne jamais citer
>    2027 d'abord**)*
> 6. « Comment prouvez-vous aujourd'hui qu'elle ne répond qu'aux commandes prévues ? »
>    *(→ H2, la question centrale du segment)*
> 7. « Qui regarde cette preuve ? Un organisme notifié, un client, personne ? »
> 8. « Qu'est-ce qui vous a été demandé de produire la dernière fois, et combien de temps
>    cela a pris ? »

### 18–25 min — ce qu'ils achèteraient, et avec quel argent

> 9. « Qu'est-ce qui vous serait le plus utile, s'il existait ? » *(→ H4, question ouverte,
>    laisser le silence)*
> 10. « Quand vous avez besoin d'une compétence que vous n'avez pas en interne, sous quelle
>     forme l'achetez-vous ? » *(→ H3 : composant, prestation, ou rien)*
> 11. « Qui signe une dépense comme celle-là, et à quel moment de l'année ? »
> 12. « Pour en être sûr, est-ce que ça se teste sur un banc, ou est-ce qu'il faut la
>     machine ? » *(→ H5)*

### 25–30 min — montrer, et seulement maintenant

Une page, deux minutes, les limites d'abord :

> « Voilà où j'en suis. Ce n'est pas certifié, ce n'est pas une fonction de sûreté, ça
> n'a jamais volé ni roulé sur une vraie machine. Ce que ça fait : un ordre refusé ne
> produit aucune action, et il y a une trace de chaque refus que personne ne peut
> réécrire sans que ça se voie. »

Puis **une** question de clôture, qui est le vrai test :

> « Est-ce que ça répond à quelque chose chez vous, ou pas du tout ? »

Et, si et seulement si la réponse est oui :

> « Qu'est-ce qu'il faudrait que je vous montre pour que ça devienne sérieux ? »

### Après l'appel, dans les 10 minutes

Noter, dans cet ordre : **leurs mots** (verbatim, pas reformulés) ; laquelle des cinq
hypothèses a avancé ; ce qui a **contredit** une attente. Un entretien qui ne contredit
rien a probablement été mal conduit.

---

## 4. Tableau de décision

| Hypothèse | Réfutée si (sur 15 entretiens) | Conséquence si réfutée |
|---|---|---|
| **H1** date au plan | < 5 citent une date | le déclencheur daté n'en est pas un → S1 perd son avantage sur S2 |
| **H2** pas de méthode de preuve | ≥ 8 ont une méthode suffisante | le problème est déjà résolu → **arrêter S1** |
| **H3** dossier, pas composant | ≥ 8 exigent un composant certifié | **S1 est fermé** pour une personne seule → basculer sur S2 |
| **H4** le journal parle | < 5 mentionnent une trace spontanément | la partie I est mon choix, pas leur besoin → réécrire le discours autour de C2 (arrêt) |
| **H5** banc suffisant | ≥ 8 exigent la machine | il faut un partenaire matériel **avant** tout développement |

**Règle d'arrêt :** si **H2 ou H3** est réfutée, ne pas continuer à développer pour S1.
Deux hypothèses réfutées parmi H1, H4, H5 imposent de réécrire `POSITIONING.md` avant de
coder autre chose.

**Taille d'échantillon :** 15 entretiens est un seuil de **décision**, pas de
significativité statistique — il n'y en a aucune ici et il ne faut pas en revendiquer.
Les seuils ci-dessus sont choisis pour qu'un résultat franc soit lisible ; un résultat
partagé (6 ou 7) veut dire « continuer à écouter », pas « c'est validé ».

---

## 5. Vingt types d'organisations à contacter, avec la fonction visée

Des **types**, pas des noms : aucune recherche d'entreprises n'a été faite, et inventer
des noms serait une donnée fabriquée. La fonction visée est partie du test : dans S1
l'acheteur est le **fabricant**, et viser le RSSI est l'erreur classique.

| # | Type d'organisation | Fonction à viser | Hypothèse qu'il teste le mieux |
|---|---|---|---|
| 1 | Fabricant d'AMR / AGV de manutention | Responsable conformité produit | H1, H2 |
| 2 | Fabricant de chariots élévateurs automatisés | Ingénieur système sécurité machine | H2, H5 |
| 3 | Fabricant de robots agricoles (désherbage, récolte) | Directeur technique | H1, H5 |
| 4 | Fabricant d'engins de chantier télécommandés | Responsable homologation | H1, H2 |
| 5 | Fabricant de robots de nettoyage industriel | Chef de produit | H3, H4 |
| 6 | Fabricant de véhicules de mine automatisés | Ingénieur sûreté de fonctionnement | H2, H4 |
| 7 | Fabricant de plateformes mobiles pour l'intralogistique | Architecte embarqué | H3, H5 |
| 8 | Intégrateur d'AMR en entrepôt | Responsable projet / avant-vente | H3, H4 |
| 9 | Intégrateur robotique agricole | Directeur des opérations | H5 |
| 10 | Organisme notifié (machines) | Auditeur / responsable de schéma | H2 — **ce qu'il accepte comme preuve** |
| 11 | Laboratoire d'essais accrédité (sécurité machine) | Ingénieur d'essais | H2, H3 |
| 12 | Cabinet de conseil en conformité machine | Consultant senior | H1 — **ce que ses clients demandent** |
| 13 | Fournisseur de commande de sécurité (automates de sécurité) | Responsable partenariats | H3 — concurrent ou canal |
| 14 | Fournisseur de modules radio industriels | Ingénieur applications | H5 |
| 15 | Exploitant de flotte logistique interne (grand entrepôt) | Responsable maintenance / HSE | H4 — **l'incident vécu** |
| 16 | Exploitant agricole de grande taille | Responsable matériel | H4, H5 |
| 17 | Port ou terminal à conteneurs automatisé | Responsable automatisation | H2, H4 |
| 18 | Université ou institut de recherche en robotique mobile | Enseignant-chercheur, responsable de plateforme | H5 — **accès à une machine** |
| 19 | Assureur de matériel industriel | Souscripteur / ingénieur prévention | H4 — qui paie l'absence de trace |
| 20 | Autorité de surveillance du marché | Inspecteur | H2 — **ce qu'elle exigera vraiment en 2027** |

**Trois de ces vingt ne sont pas des clients** et sont les plus utiles : l'organisme
notifié (10), l'autorité de surveillance (20) et le laboratoire (11) savent ce qui sera
accepté comme preuve. Un seul entretien avec eux peut réfuter H2 ou H3 plus vite que dix
entretiens avec des fabricants.

**L'université (18)** est le chemin le plus court vers une machine réelle, qui est le
blocage matériel du segment.

---

## 6. Ce que ce document ne fait pas

- Il ne contient **aucun nom d'entreprise, aucun contact, aucune donnée de marché** : rien
  de cela n'a été recherché.
- Il ne contient **aucune estimation de taux de réponse ni de durée de cycle de vente** —
  ce serait inventé.
- Les seuils du §4 sont des **choix de décision**, posés pour être explicites et
  critiquables, pas des résultats.
- Aucun entretien n'a eu lieu. **Les cinq hypothèses sont toutes au statut « non
  testée ».**
