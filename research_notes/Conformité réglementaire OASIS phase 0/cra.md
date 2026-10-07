# Cyber Resilience Act — règlement (UE) 2024/2847 : exigences essentielles et obligations de signalement

> **Méthode** : le texte intégral FR et EN du règlement a été téléchargé depuis EUR-Lex
> (`https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847` et la version EN
> équivalente) puis cité verbatim. Toutes les citations ci-dessous sont des extraits littéraux de
> la version FR publiée au *Journal officiel*, série L, **2024/2847 du 20.11.2024**. Les deux
> versions linguistiques ont été recoupées sur les points sensibles.
>
> **Références EUR-Lex stables** :
> - Page du règlement (FR) : https://eur-lex.europa.eu/legal-content/FR/TXT/?uri=OJ:L_202402847
> - ELI : https://eur-lex.europa.eu/eli/reg/2024/2847/oj
> - HTML intégral FR : https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847
> - HTML intégral EN : https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=OJ:L_202402847
>
> **Intitulé officiel** : « RÈGLEMENT (UE) 2024/2847 DU PARLEMENT EUROPÉEN ET DU CONSEIL du
> 23 octobre 2024 concernant des exigences de cybersécurité horizontales pour les produits
> comportant des éléments numériques et modifiant les règlements (UE) n° 168/2013 et (UE) 2019/1020
> et la directive (UE) 2020/1828 (règlement sur la cyberrésilience) ».
>
> **Avertissement de périmètre** : ces notes décrivent uniquement l'exigence réglementaire externe.
> Aucune analyse d'applicabilité à un produit particulier n'est faite ici.

---

## Q1 — Annexe I : texte exact des exigences essentielles (partie I) et de la gestion des vulnérabilités (partie II)

### Takeaway

L'annexe I est courte et close : **partie I = 2 points** (point 1 obligation générale de conception
par les risques ; point 2 = 13 sous-exigences a) à m) applicables « le cas échéant » sur la base de
l'évaluation de risque de l'article 13 §2) ; **partie II = 8 points** numérotés 1) à 8) imposant au
fabricant un processus de gestion des vulnérabilités (SBOM, correction sans retard, tests, divulgation,
CVD, canal de contact, distribution sécurisée, diffusion gratuite). Lien précis :
https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847 (ancre « ANNEXE I »).

### Cited Findings

Titre : **« ANNEXE I — EXIGENCES ESSENTIELLES DE CYBERSÉCURITÉ »**, puis
**« Partie I — Exigences de cybersécurité relatives aux propriétés des produits comportant des éléments numériques »** — [EUR-Lex, règlement (UE) 2024/2847, annexe I](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Annexe I, partie I, point 1)** (texte exact) :

> « Les produits comportant des éléments numériques sont conçus, développés et fabriqués de manière à garantir un niveau de cybersécurité approprié en fonction des risques. »
> — [annexe I, partie I, point 1](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Annexe I, partie I, point 2)** — chapeau et les 13 sous-points (texte exact) :

> « Sur la base de l'évaluation des risques de cybersécurité visée à l'article 13, paragraphe 2, les produits comportant des éléments numériques doivent, le cas échéant:
> a) être mis à disposition sur le marché sans vulnérabilité exploitable connue;
> b) être mis à disposition sur le marché avec une configuration de sécurité par défaut, sauf accord contraire entre le fabricant et l'entreprise utilisatrice en ce qui concerne un produit sur mesure comportant des éléments numériques, y compris la possibilité de réinitialiser le produit à son état d'origine;
> c) être conçus de façon à ce leurs vulnérabilités puissent être corrigées par des mises à jour de sécurité, y compris, le cas échéant, par des mises à jour automatiques de sécurité régulières activées par défaut, mais faciles à désactiver, par la communication aux utilisateurs des mises à jour disponibles et par la possibilité de les différer temporairement;
> d) assurer la protection contre les accès non autorisés par des mécanismes de contrôle appropriés, y compris, mais sans s'y limiter, par des systèmes d'authentification, d'identité ou de gestion des accès et signaler tout accès non autorisé;
> e) protéger la confidentialité des données stockées, transmises ou traitées de toute autre manière, à caractère personnel ou autres, par exemple en chiffrant les données pertinentes au repos ou en transit au moyen de mécanismes de pointe et par d'autres moyens techniques;
> f) protéger l'intégrité des données stockées, transmises ou traitées de toute autre manière, à caractère personnel ou autres, des commandes, des programmes et de la configuration contre toute manipulation ou modification non autorisée par l'utilisateur et signaler les corruptions;
> g) ne traiter que les données, à caractère personnel ou autres, qui sont adéquates, pertinentes et limitées à ce qui est nécessaire au regard de la finalité prévue du produit comportant des éléments numériques (minimisation des données);
> h) protéger la disponibilité des fonctions essentielles et de base, notamment après un incident, y compris par des mesures de résilience et d'atténuation face aux attaques par déni de service;
> i) réduire au maximum les répercussions négatives générées par les produits eux-mêmes ou par les appareils connectés sur la disponibilité des services fournis par d'autres dispositifs ou réseaux;
> j) être conçus, développés et fabriqués de manière à limiter les surfaces d'attaque, y compris les interfaces externes;
> k) être conçus, développés et fabriqués de manière à réduire les répercussions d'un incident, en utilisant des mécanismes et des techniques appropriés de limitation de l'exploitation de failles;
> l) fournir des informations relatives à la sécurité en enregistrant et en surveillant les activités internes pertinentes, y compris l'accès ou la modification des données, des services ou des fonctions, tout en laissant à l'utilisateur la possibilité de désactiver le mécanisme;
> m) donner aux utilisateurs la possibilité de supprimer facilement, en toute sécurité et de manière permanente toutes les données et tous les paramètres et, lorsque ces données peuvent être transférées vers d'autres produits ou systèmes, veiller à ce que cela puisse se faire de manière sécurisée. »
> — [annexe I, partie I, point 2](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Annexe I, partie II — « Exigences relatives à la gestion des vulnérabilités »**, chapeau
« Les fabricants des produits comportant des éléments numériques: » puis points 1) à 8) (texte exact) :

> « 1) recensent et documentent les vulnérabilités et les composants des produits, notamment par l'établissement d'une nomenclature des logiciels dans un format couramment utilisé et lisible par machine couvrant au moins les dépendances de niveau supérieur des produits;
> 2) gèrent et corrigent sans retard les vulnérabilités qui touchent les produits comportant des éléments numériques, y compris par des mises à jour de sécurité; lorsque cela est techniquement possible, de nouvelles mises à jour de sécurité sont fournies séparément des mises à jour de fonctionnalité;
> 3) soumettent régulièrement les produits comportant des éléments numériques à des tests et examens de sécurité efficaces;
> 4) dès la publication d'une mise à jour de sécurité, communiquent sur les vulnérabilités corrigées, en publiant notamment une description des vulnérabilités, des informations permettant aux utilisateurs d'identifier le produit comportant des éléments numérique concerné, les conséquences de ces vulnérabilités, leur gravité et des informations claires et accessibles aidant les utilisateurs à y remédier; dans des cas dûment justifiés, lorsque les fabricants considèrent que les risques pour la sécurité liés à la publication l'emportent sur les avantages en matière de sécurité, ils peuvent retarder la publication des informations relatives à une vulnérabilité corrigée jusqu'à ce que les utilisateurs aient eu la possibilité d'appliquer le correctif adapté;
> 5) mettent en place et appliquent une politique de divulgation coordonnée des vulnérabilités;
> 6) prennent des mesures pour faciliter le partage d'informations sur les vulnérabilités potentielles de leurs produits comportant des éléments numériques ainsi que des composants tiers contenus dans ces produits, y compris en fournissant une adresse de contact pour le signalement des vulnérabilités découvertes dans les produits concernés;
> 7) prévoient des mécanismes de distribution sécurisée des mises à jour pour les produits comportant des éléments numériques afin de garantir que les vulnérabilités soient corrigées ou atténuées rapidement et, le cas échéant, automatisent les mises à jour de sécurité;
> 8) veillent à ce que, lorsque des correctifs ou des mises à jour de sécurité sont disponibles pour remédier à des problèmes de sécurité constatés, ils soient diffusés sans retard et, sauf accord contraire entre un fabricant et un utilisateur professionnel en ce qui concerne un produit sur mesure comportant des éléments numériques, gratuitement et accompagnées de messages consultatifs fournissant aux utilisateurs les informations pertinentes, y compris sur les éventuelles mesures à prendre. »
> — [annexe I, partie II, points 1 à 8](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

Mécanisme de rattachement : l'article 13 §1 dispose que « *Lorsqu'ils mettent sur le marché un
produit comportant des éléments numériques, les fabricants s'assurent que ce produit a été conçu,
développé et fabriqué conformément aux exigences essentielles de cybersécurité énoncées à
l'annexe I, partie I* » ; l'article 13 §8 premier alinéa renvoie à la partie II pour la période
d'assistance — [article 13 §1 et §8](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

L'article 13 §3 impose que l'évaluation de risque « *indique si et, dans l'affirmative, de quelle
manière, les exigences de sécurité énoncées à l'annexe I, partie I, point 2), sont applicables au
produit* » ; l'article 13 §4 ajoute : « *Lorsque certaines exigences essentielles de cybersécurité
ne sont pas applicables au produit comportant des éléments numériques, le fabricant fait figurer
une justification claire dans cette documentation technique.* »
— [article 13 §3 et §4](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

### Inferences

- Les 13 sous-exigences de la partie I point 2 ne sont pas inconditionnelles : elles s'appliquent
  « le cas échéant » (*where applicable*), et leur non-applicabilité doit être **justifiée par
  écrit dans la documentation technique** (art. 13 §4). Le point 1 de la partie I, lui, n'est pas
  conditionné.
- La partie II est rédigée au présent de l'indicatif sur le sujet « les fabricants » : il s'agit
  d'obligations de processus organisationnel, pas de propriétés produit, et elles courent pendant
  toute la période d'assistance (art. 13 §8 al. 1).

### Gaps

- L'annexe I ne définit pas de niveau de preuve ni de métriques ; cela relèvera des normes
  harmonisées (voir Q7).

---

## Q2 — Article 14 : obligations de signalement (à qui, quoi, délais)

### Takeaway

L'article 14 est intitulé **« Obligations en matière de communication d'informations incombant aux
fabricants »**. Il crée **deux canaux parallèles obligatoires** (vulnérabilité activement exploitée ;
incident grave) notifiés **simultanément au CSIRT désigné comme coordinateur et à l'ENISA**, via la
**plateforme unique de signalement** de l'article 16, selon un triptyque **24 h / 72 h / rapport
final** — le rapport final étant à **14 jours** pour les vulnérabilités et à **1 mois** pour les
incidents.

### Cited Findings

**Article 14 §1** (texte exact) :

> « Un fabricant notifie toute vulnérabilité activement exploitée contenue dans le produit comportant des éléments numériques dont il prend connaissance simultanément au CSIRT désigné comme coordinateur conformément au paragraphe 7 du présent article, et à l'ENISA. Le fabricant notifie cette vulnérabilité activement exploitée par l'intermédiaire de la plateforme unique de signalement établie en vertu de l'article 16. »
> — [article 14 §1](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 14 §2 — vulnérabilité activement exploitée, les trois échéances** (texte exact) :

> « Aux fins de la notification visée au paragraphe 1, le fabricant soumet:
> a) une alerte précoce de vulnérabilité activement exploitée, sans retard injustifié et, en tout état de cause, **au plus tard 24 heures** après en avoir eu connaissance, en indiquant, le cas échéant, les États membres sur le territoire desquels il a connaissance que son produit comportant des éléments numériques a été mis à disposition;
> b) à moins que les informations pertinentes n'aient déjà été communiquées, une notification de vulnérabilité, sans retard injustifié et, en tout état de cause, **au plus tard 72 heures** après avoir eu connaissance de la vulnérabilité activement exploitée, fournissant les informations générales disponibles sur le produit comportant des éléments numériques concerné, la nature générale de l'exploitation et de la vulnérabilité concernée, ainsi que toute mesure corrective ou d'atténuation prise et les mesures correctives ou d'atténuation que les utilisateurs peuvent prendre, et précisant, s'il y a lieu, le degré de sensibilité qu'il attribue aux informations notifiées;
> c) à moins que les informations pertinentes n'aient déjà été communiquées, un rapport final, **au plus tard 14 jours après la mise à disposition d'une mesure de correction ou d'atténuation**, comprenant au moins les éléments suivants:
> i) une description de la vulnérabilité, y compris de sa gravité et de ses répercussions;
> ii) le cas échéant, des informations concernant tout acteur malveillant ayant exploité ou exploitant la vulnérabilité;
> iii) des précisions concernant la mise à jour de sécurité ou les autres mesures correctives qui ont été mises en place pour remédier à la vulnérabilité. »
> *(le gras est ajouté ; le texte officiel n'est pas en gras)*
> — [article 14 §2](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 14 §3** (incidents graves) : « *Un fabricant notifie tout incident grave ayant des
répercussions sur la sécurité du produit comportant des éléments numériques dont il prend
connaissance simultanément au CSIRT désigné comme coordinateur conformément au paragraphe 7 du
présent article et à l'ENISA.* »
— [article 14 §3](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 14 §4 — incident grave, les trois échéances** (texte exact) :

> « a) une alerte précoce d'incident grave ayant des répercussions sur la sécurité du produit comportant des éléments numériques, sans retard injustifié et, en tout état de cause, **au plus tard 24 heures** après en avoir eu connaissance, indiquant, au minimum, si l'incident pourrait avoir été causé par des actes illicites ou malveillants et, le cas échéant, les États membres sur le territoire desquels il a connaissance que son produit comportant des éléments numériques a été mis à disposition;
> b) à moins que les informations pertinentes n'aient déjà été communiquées, une notification d'incident, sans retard injustifié et, en tout état de cause, **au plus tard 72 heures** après avoir eu connaissance de l'incident, fournissant les informations générales, lorsqu'elles sont disponibles, sur la nature de l'incident, l'évaluation initiale de l'incident, ainsi que toute mesure corrective ou d'atténuation prise et les mesures correctives ou d'atténuation que les utilisateurs peuvent prendre, et précisant, le cas échéant, le degré de sensibilité qu'il attribue aux informations notifiées;
> c) à moins que les informations pertinentes n'aient déjà été communiquées, **dans un délai d'un mois à compter de la présentation de la notification d'incident visée au point b)**, un rapport final comprenant au moins les éléments suivants:
> i) une description détaillée de l'incident, y compris de sa gravité et de ses répercussions;
> ii) le type de menace ou la cause profonde qui a probablement déclenché l'incident;
> iii) les mesures d'atténuation appliquées et en cours. »
> — [article 14 §4](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 14 §5 — définition de l'incident « grave »** (texte exact) :

> « Aux fins du paragraphe 3, un incident ayant des répercussions sur la sécurité du produit comportant des éléments numériques est considéré comme grave lorsque:
> a) il entache ou est susceptible d'entacher la capacité d'un produit comportant des éléments numériques à protéger la disponibilité, l'authenticité, l'intégrité ou la confidentialité de données ou fonctions sensibles ou importantes; ou
> b) il a conduit ou est susceptible de conduire à l'introduction ou à l'exécution d'un code malveillant dans un produit comportant des éléments numériques ou dans le réseau et les systèmes d'information d'un utilisateur du produit comportant des éléments numériques. »
> — [article 14 §5](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 14 §6** : « *Si nécessaire, le CSIRT désigné comme coordinateur qui reçoit initialement la
notification peut demander au fabricant de fournir un rapport intermédiaire de situation* ».
— [article 14 §6](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 14 §7 — quel CSIRT ?** Les notifications « *sont soumises par l'intermédiaire de la
plateforme unique de signalement visée à l'article 16 en utilisant l'un des points finaux de
notification électronique visés à l'article 16, paragraphe 1. La notification est soumise au moyen
du point final de notification électronique du CSIRT désigné comme coordinateur de l'État membre
dans lequel le fabricant a son établissement principal dans l'Union et est simultanément mise à la
disposition de l'ENISA.* » Définition de l'établissement principal : « *l'État membre où sont
principalement prises les décisions relatives à la cybersécurité des produits* », à défaut
« *l'État membre où le fabricant concerné possède l'établissement comptant le plus grand nombre de
salariés dans l'Union* ». Sans établissement principal dans l'Union, cascade a) État du mandataire
pour le plus grand nombre de produits → b) État de l'importateur → c) État du distributeur → d) État
comptant le plus grand nombre d'utilisateurs.
— [article 14 §7](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 14 §8 — information des utilisateurs** (obligation distincte de la notification aux
autorités, texte exact) :

> « Après avoir pris connaissance d'une vulnérabilité activement exploitée ou d'un incident grave ayant des répercussions sur la sécurité du produit comportant des éléments numériques, le fabricant informe les utilisateurs du produit comportant des éléments numériques touchés et, s'il y a lieu, tous les utilisateurs de ladite vulnérabilité ou dudit incident et, si nécessaire, de toute mesure corrective ou d'atténuation des risques que les utilisateurs peuvent mettre en place pour atténuer les répercussions de cette vulnérabilité ou de cet incident, s'il y a lieu dans un format structuré, lisible par machine pouvant être facilement traité automatiquement. Lorsque le fabricant n'informe pas les utilisateurs du produit comportant des éléments numériques en temps utile, les CSIRT notifiés désignés comme coordinateurs peuvent fournir ces informations aux utilisateurs lorsqu'ils le jugent proportionné et nécessaire pour prévenir ou atténuer les répercussions de cette vulnérabilité ou de cet incident. »
> — [article 14 §8](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 14 §9 et §10 — actes à venir** : §9 « *Au plus tard le 11 décembre 2025, la Commission
adopte des actes délégués […] pour […] préciser[…] les conditions d'application des motifs ayant
trait à la cybersécurité en lien avec les retards de diffusion des notifications prévus à
l'article 16, paragraphe 2* » ; §10 « *La Commission peut, par voie d'actes d'exécution, préciser
plus en détail le format et les procédures des notifications visées au présent article ainsi qu'aux
articles 15 et 16.* »
— [article 14 §9 et §10](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 15 — signalement volontaire** : fabricants « *mais aussi d'autres personnes physiques ou
morales* » peuvent notifier volontairement vulnérabilités, cybermenaces, incidents et « *incidents
évités* » ; §5 : « *un signalement volontaire n'a pas pour effet d'imposer à la personne physique ou
morale à l'origine de la notification des obligations supplémentaires auxquelles elle n'aurait pas
été soumise si elle n'avait pas fait la notification* ».
— [article 15](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Exonération d'amende pour les micro/petites entreprises sur le délai de 24 h** : article 64 §10
a) : les amendes administratives ne s'appliquent pas « *aux fabricants considérés comme des
microentreprises ou des petites entreprises en cas de non-respect du délai visé à l'article 14,
paragraphe 2, point a), ou à l'article 14, paragraphe 4, point a)* ».
— [article 64 §10 a)](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Confirmation administrative des délais et de la plateforme** : la page officielle de la Commission
« Cyber Resilience Act – Reporting obligations » reprend : « early warning: within 24 hours of
becoming aware », « full notification: within 72 hours », « final report for vulnerabilities: no
later than 14 days after a corrective measure is available », « final report for incidents: within a
month from the 72-hour notification », et indique que la **CRA Single Reporting Platform (SRP)**
établie par l'ENISA est « operational as of 11 September 2026 »
— [Commission européenne, CRA – Reporting obligations](https://digital-strategy.ec.europa.eu/en/policies/cra-reporting)

### Inferences

- Le déclencheur est la **connaissance** (« dont il prend connaissance » / « après en avoir eu
  connaissance »), pas la publication ni la confirmation : le compteur de 24 h démarre à la prise de
  connaissance, et le texte ne prévoit pas de seuil de criticité pour les vulnérabilités (seul
  critère : « activement exploitée »).
- Le rapport final « vulnérabilité » est ancré sur la **disponibilité du correctif** (14 j après),
  alors que le rapport final « incident » est ancré sur la **notification à 72 h** (1 mois après) :
  deux horloges différentes, à modéliser séparément dans une procédure interne.
- L'obligation de l'article 14 §8 (informer les utilisateurs) est **autonome** et sans délai chiffré
  (« en temps utile ») ; le défaut d'information ouvre la faculté pour le CSIRT d'informer
  directement les utilisateurs.

### Gaps

- Aucun acte d'exécution au titre de l'article 14 §10 (format/procédures de notification) n'a pu
  être vérifié à la source : la page officielle « CRA – Reporting obligations » ne fournit « no
  information » sur ce point (**non vérifié à la source**).
- La page de la Commission indique qu'« on 11 December 2025, the Commission adopted a delegated act
  specifying terms for delaying dissemination based on cybersecurity grounds » (acte délégué de
  l'art. 14 §9) ; **la référence EUR-Lex / le numéro de cet acte délégué n'a pas été vérifiée à la
  source** dans cette recherche. Source de l'information : la page officielle
  [CRA – Reporting obligations](https://digital-strategy.ec.europa.eu/en/policies/cra-reporting).

---

## Q3 — Dates exactes d'application (vérifiées à la source)

### Takeaway

Publication au JO le **20.11.2024** ; entrée en vigueur le **10 décembre 2024** (20e jour) ;
**article 14 applicable depuis le 11 septembre 2026** ; **chapitre IV (art. 35 à 51) depuis le
11 juin 2026** ; **application pleine le 11 décembre 2027**. Au **7 octobre 2026**, seules les
obligations de signalement de l'article 14 et le chapitre IV sont en vigueur ; les exigences
essentielles de l'annexe I, le marquage CE, l'article 13 et le régime des intendants de logiciels
ouverts (art. 24) ne le sont **pas encore**.

### Cited Findings

**Article 71 — « Entrée en vigueur et application »** (texte exact) :

> « 1. Le présent règlement entre en vigueur le vingtième jour suivant celui de sa publication au *Journal officiel de l'Union européenne*.
> 2. Le présent règlement est applicable à partir du **11 décembre 2027**.
> Toutefois, l'article 14 est applicable à partir du **11 septembre 2026** et le chapitre IV (articles 35 à 51) à partir du **11 juin 2026**. »
> — [article 71](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

En-tête officiel du JO dans le fichier EUR-Lex : « Journal officiel de l'Union européenne / Série L /
2024/2847 / **20.11.2024** », acte « du 23 octobre 2024 », « Fait à Strasbourg, le 23 octobre 2024 »
— [EUR-Lex, OJ L 2024/2847](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

Confirmation administrative de la date d'entrée en vigueur : « The CRA entered into force on
**10 December 2024** » ; « reporting obligations to apply as of **11 September 2026** » ; « The main
obligations introduced by the Act will apply from **11 December 2027** »
— [Commission européenne, page officielle Cyber Resilience Act](https://digital-strategy.ec.europa.eu/en/policies/cyber-resilience-act)

**Article 69 — dispositions transitoires** (texte exact, FR) :

> « 1. Les attestations d'examen UE de type et les décisions d'approbation délivrées en ce qui concerne les exigences de cybersécurité applicables aux produits comportant des éléments numériques qui sont soumis à d'autres législations d'harmonisation de l'Union restent valables jusqu'au **11 juin 2028**, à moins qu'elles n'expirent avant cette date […]
> 2. Les produits comportant des éléments numériques qui ont été mis sur le marché **avant le 11 décembre 2027** ne sont soumis aux exigences énoncées dans le présent règlement que si, à compter de cette date, ces produits font l'objet d'une **modification substantielle**.
> 3. Par dérogation au paragraphe 2 du présent article, les obligations prévues à l'article 14 s'appliquent à tous les produits comportant des éléments numériques relevant du champ d'application du présent règlement qui ont été mis sur le marché le 11 décembre 2027. »
> — [article 69](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

⚠️ **Divergence linguistique relevée au JO sur l'article 69 §3** : la version FR publiée écrit
« *mis sur le marché **le** 11 décembre 2027* », alors que la version EN publiée écrit « *that have
been placed on the market **before** 11 December 2027* »
— [version EN, article 69(3)](https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=OJ:L_202402847)
vs [version FR, article 69(3)](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847).
La lecture EN (« before ») est celle qui donne un sens à la dérogation au §2 et correspond à la
pratique administrative (art. 14 applicable dès 2026, y compris au parc déjà sur le marché).

Autres dates du texte, vérifiées à la source :
- article 14 §9 : acte délégué « au plus tard le **11 décembre 2025** » — [article 14 §9](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)
- article 70 §2 : rapport d'évaluation de la plateforme unique de signalement « au plus tard le **11 septembre 2028** » — [article 70 §2](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)
- article 70 §1 : rapport d'évaluation et de réexamen « au plus tard le **11 décembre 2030** et tous les quatre ans par la suite » — [article 70 §1](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)
- Pour les intendants de logiciels ouverts, la Commission indique une entrée en application des obligations de signalement au **11 décembre 2027** « per Article 71(2) » (l'article 24, qui étend l'art. 14 aux intendants, n'étant lui-même applicable qu'à cette date) — [Commission, CRA – Reporting obligations](https://digital-strategy.ec.europa.eu/en/policies/cra-reporting)

### Inferences

- **État au 7 octobre 2026** : applicables → art. 14 (depuis 11.09.2026) et chapitre IV, art. 35‑51
  (notification des organismes d'évaluation de la conformité, depuis 11.06.2026). Non encore
  applicables → art. 13 (obligations des fabricants), annexe I (exigences essentielles), annexes II
  et VII (information utilisateur, documentation technique), marquage CE, art. 24 (intendants),
  régime de sanctions lié aux exigences essentielles, c'est-à-dire l'essentiel du règlement →
  **11 décembre 2027**.
- Conséquence pratique : un produit déjà sur le marché aujourd'hui est, dès à présent, dans le champ
  de l'obligation de **signalement** (art. 69 §3 lu avec la version EN), mais pas encore dans celui
  des exigences essentielles tant qu'il ne subit pas de modification substantielle après le
  11 décembre 2027.

### Gaps

- La date d'entrée en vigueur (10 décembre 2024) est un calcul (20e jour après le 20.11.2024)
  confirmé par la page officielle de la Commission, mais le texte lui-même ne la mentionne pas
  explicitement.

---

## Q4 — SBOM : ce que le texte demande exactement, format imposé, destinataire

### Takeaway

Le CRA exige une **nomenclature des logiciels** (*software bill of materials*) « dans un format
couramment utilisé et lisible par machine », couvrant « **au moins les dépendances de niveau
supérieur** » — **aucun format nommé** (ni CycloneDX ni SPDX dans le texte). Elle **n'est pas à
rendre publique** (considérant 77), elle fait partie de la **documentation technique** (annexe VII)
et n'est communiquée à une autorité de surveillance du marché que **sur demande motivée**.

### Cited Findings

**Annexe I, partie II, point 1)** (texte exact — l'exigence de fond) :

> « recensent et documentent les vulnérabilités et les composants des produits, notamment par l'établissement d'une nomenclature des logiciels dans un format couramment utilisé et lisible par machine couvrant au moins les dépendances de niveau supérieur des produits; »
> — [annexe I, partie II, point 1](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

Version EN de référence : « *identify and document vulnerabilities and components contained in
products with digital elements, including by drawing up a software bill of materials in a commonly
used and machine-readable format covering at the very least the top-level dependencies of the
products* »
— [annexe I, partie II, point 1, version EN](https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=OJ:L_202402847)

**Définition légale (article 3, point 36 — « nomenclature des logiciels »)** :

> « un document officiel contenant les détails et les relations avec la chaîne d'approvisionnement des différents composants utilisés dans la fabrication d'un produit comportant des éléments numériques; »
> — [article 3, définitions](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)
> *(numéro de point à revérifier : la définition apparaît dans la liste de l'article 3 mais son numéro exact n'a pas été isolé dans l'extraction — **non vérifié à la source** pour le numéro de point seul, le libellé est verbatim.)*

**Pas de publication obligatoire — considérant 77** (texte exact, dernière phrase) :

> « Afin de faciliter l'analyse de la vulnérabilité, les fabricants devraient répertorier et documenter les composants contenus dans les produits comportant des éléments numériques, notamment en établissant une nomenclature des logiciels. […] **Les fabricants ne devraient pas être tenus de rendre publique la nomenclature des logiciels.** »
> — [considérant 77](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Mise à disposition de l'utilisateur : facultative — annexe II, point 9** :

> « Si le fabricant décide de mettre à la disposition de l'utilisateur la nomenclature des logiciels, des informations sur l'endroit où celle-ci peut être consultée. »
> — [annexe II, point 9](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Place dans la documentation technique — annexe VII** : le point 2 b) exige « *les informations et
spécifications nécessaires concernant le processus de gestion des vulnérabilités mis en place par le
fabricant, **en ce compris la nomenclature des logiciels**, la politique coordonnée de divulgation
des vulnérabilités, la preuve de la fourniture d'une adresse de contact pour le signalement des
vulnérabilités et une description des solutions techniques choisies pour la distribution sécurisée
des mises à jour* » ; et le point 8 : « *le cas échéant, la nomenclature des logiciels, à la suite
d'une **demande motivée d'une autorité de surveillance du marché**, pour autant que celle-ci soit
nécessaire pour permettre à cette autorité de vérifier le bon respect des exigences essentielles de
cybersécurité énoncées à l'annexe I* »
— [annexe VII, points 2 b) et 8](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Format : habilitation, pas de format imposé aujourd'hui — article 13 §24** :

> « La Commission peut, par voie d'actes d'exécution tenant compte des normes et bonnes pratiques européennes et internationales, préciser le format et les éléments de la nomenclature des logiciels visée à l'annexe I, partie II, point 1). Ces actes d'exécution sont adoptés en conformité avec la procédure d'examen visée à l'article 62, paragraphe 2. »
> — [article 13 §24](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Usage macro possible des SBOM — article 13 §25** : l'ADCO peut décider d'une évaluation de
dépendance à l'échelle de l'Union ; « *les autorités de surveillance du marché peuvent demander aux
fabricants de ces catégories de produits […] de fournir les nomenclatures des logiciels pertinentes
du matériel visées à l'annexe I, partie II, point 1)* », les informations étant ensuite transmises à
l'ADCO « *anonymisées et agrégées* »
— [article 13 §25](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847) ; voir
aussi le considérant 22 (« *Afin de préserver la confidentialité des nomenclatures des logiciels…* »)
— [considérant 22](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

### Inferences

- **Aucun format n'est imposé par le règlement.** Les noms « CycloneDX » et « SPDX » n'apparaissent
  nulle part dans le texte (recherche plein texte sur la version FR et EN téléchargées : 0
  occurrence). Le critère légal est double et qualitatif : « couramment utilisé » **et** « lisible
  par machine ». CycloneDX et SPDX satisfont a priori ces deux critères, mais c'est une déduction,
  pas une prescription du texte.
- Le périmètre minimal est **les dépendances de niveau supérieur** (« top-level dependencies ») : le
  texte n'exige pas un arbre transitif complet, tout en fixant ce niveau comme un plancher
  (« au moins » / « at the very least »).
- La SBOM est un **livrable interne** opposable à l'autorité sur demande motivée, pas un livrable
  public ni un livrable client par défaut. Trois destinataires possibles, par ordre d'obligation :
  (1) dossier technique interne — obligatoire ; (2) autorité de surveillance du marché — sur demande
  motivée ; (3) utilisateur ou public — facultatif.

### Gaps

- Aucun acte d'exécution au titre de l'article 13 §24 (format et éléments de la SBOM) n'a été
  identifié comme adopté. **Non vérifié à la source** : la recherche n'a pas trouvé de page
  officielle confirmant ou infirmant l'adoption d'un tel acte à la date du 7 octobre 2026.
- Le numéro exact du point de l'article 3 portant la définition de « nomenclature des logiciels »
  n'a pas été isolé (le libellé cité est verbatim).

---

## Q5 — Durée de support : période d'assistance et mises à jour de sécurité

### Takeaway

La « **période d'assistance** » est définie à l'article 3 et encadrée à l'article 13 §8 : fixée par
le fabricant pour refléter la durée d'utilisation attendue, **au minimum 5 ans** (sauf durée
d'utilisation prévue inférieure), avec une obligation distincte de **conserver les mises à jour de
sécurité disponibles au moins 10 ans** après leur émission (art. 13 §9) et d'**afficher la date de
fin** (mois + année) au moment de l'achat (art. 13 §19).

### Cited Findings

**Définition — article 3 (point « période d'assistance »)** :

> « la période au cours de laquelle un fabricant est tenu de garantir que les vulnérabilités d'un produit comportant des éléments numériques sont traitées efficacement et conformément aux exigences essentielles de cybersécurité énoncées à l'annexe I, partie II; »
> — [article 3, définitions](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 13 §8 — texte exact des cinq alinéas** :

> « Lorsqu'ils mettent sur le marché un produit comportant des éléments numériques, et pendant la période d'assistance, les fabricants veillent à ce que les vulnérabilités de ce produit, y compris de ses composants, soient gérées efficacement et conformément aux exigences essentielles de cybersécurité énoncées à l'annexe I, partie II.
>
> Les fabricants fixent la période d'assistance de sorte qu'elle reflète la durée pendant laquelle le produit est censé pouvoir être utilisé, en tenant compte, en particulier, des attentes raisonnables des utilisateurs, de la nature du produit, y compris de son utilisation prévue, ainsi que du droit de l'Union applicable déterminant la durée de vie des produits comportant des éléments numériques. Lorsqu'ils déterminent la période d'assistance, les fabricants peuvent également tenir compte des périodes d'assistance des produits comportant des éléments numériques offrant une fonctionnalité similaire mis sur le marché par d'autres fabricants, de la disponibilité de l'environnement opérationnel, des périodes d'assistance des composants intégrés qui assurent des fonctions essentielles et proviennent de tiers, ainsi que des orientations pertinentes fournies par le groupe de coopération administrative (ADCO) institué en vertu de l'article 52, paragraphe 15, et par la Commission. Les éléments à prendre en compte pour définir la période d'assistance sont pris en compte de manière à garantir la proportionnalité.
>
> **Sans préjudice du deuxième alinéa, la période d'assistance est d'au moins cinq ans. Lorsque le produit comportant des éléments numériques est censé pouvoir être utilisé pendant moins de cinq ans, la période d'assistance correspond à la durée d'utilisation prévue.**
>
> Compte tenu des recommandations ADCO visées à l'article 52, paragraphe 16, la Commission peut adopter des actes délégués conformément à l'article 61 afin de compléter le présent règlement en précisant la période d'assistance minimale pour des catégories de produits spécifiques lorsque les données de surveillance du marché indiquent que les périodes d'assistance fixées sont insuffisantes.
>
> Les fabricants font figurer dans la documentation technique visée à l'annexe VII les informations qui ont été prises en compte pour déterminer la période d'assistance du produit comportant des éléments numériques.
>
> Les fabricants disposent de politiques et de procédures appropriées, notamment les politiques de divulgation coordonnée des vulnérabilités mentionnées à l'annexe I, partie II, point 5), pour traiter et corriger les vulnérabilités potentielles du produit comportant des éléments numériques signalées par des sources internes ou externes. »
> — [article 13 §8](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 13 §9 — disponibilité des mises à jour pendant 10 ans** (texte exact) :

> « Les fabricants veillent à ce que chaque mise à jour de sécurité, visée à l'annexe I, partie II, point 8), qui a été mise à la disposition des utilisateurs au cours de la période d'assistance, reste disponible après son émission pendant dix ans au minimum ou pendant le reste de la période d'assistance, la période la plus longue étant retenue. »
> — [article 13 §9](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 13 §10 — versions logicielles successives** :

> « Lorsqu'un fabricant met sur le marché des versions ultérieures substantiellement modifiées d'un logiciel, il peut garantir la conformité avec l'exigence essentielle de cybersécurité énoncée à l'annexe I, partie II, point 2), uniquement pour la dernière version mise sur le marché, à condition que les utilisateurs des versions précédemment mises sur le marché aient accès gratuitement aux dernières versions mises sur le marché et ne doivent pas supporter des coûts supplémentaires pour adapter l'environnement matériel et logiciel dans lequel ils utilisent la version originale de ce produit. »
> — [article 13 §10](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 13 §11 — archives de versions antérieures** : « *Les fabricants peuvent conserver des
archives logicielles publiques améliorant l'accès des utilisateurs aux versions antérieures. Dans
ces cas, les utilisateurs sont clairement informés, d'une manière aisément accessible, des risques
associés à l'utilisation de logiciels non pris en charge.* »
— [article 13 §11](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 13 §19 — communication de la date de fin** (texte exact) :

> « Les fabricants veillent à ce que la date de fin de la période d'assistance visée au paragraphe 8, y compris au moins le mois et l'année, soit précisée au moment de l'achat, d'une manière claire, compréhensible et aisément accessible et, le cas échéant, sur le produit comportant des éléments numériques, son emballage ou par des moyens numériques.
> Lorsque cela est techniquement possible compte tenu de la nature du produit comportant des éléments numériques, les fabricants prévoient l'affichage d'une notification aux utilisateurs les informant que leur produit comportant des éléments numériques a atteint la fin de sa période d'assistance. »
> — [article 13 §19](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Annexe II, point 7 — information obligatoire à l'utilisateur** : « *le type d'assistance technique
en matière de sécurité proposé par le fabricant et la date de fin de la période d'assistance pendant
laquelle les utilisateurs peuvent s'attendre à recevoir des mises à jour de sécurité;* »
— [annexe II, point 7](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Annexe VII, point 4** : la documentation technique doit contenir « *les informations qui ont été
pris[es] en compte pour déterminer la période d'assistance en vertu de l'article 13, paragraphe 8* »
— [annexe VII, point 4](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Surveillance du marché — article 52 §16** : « *Les autorités de surveillance du marché contrôlent
la manière dont les fabricants ont appliqué les critères indiqués à l'article 13, paragraphe 8, en
déterminant la période d'assistance* » ; l'ADCO « *publie sous une forme accessible au public et
conviviale des statistiques pertinentes sur les catégories de produits […] y compris leur période
d'assistance moyenne […] et fournit des orientations qui comprennent des périodes d'assistance
indicatives pour les catégories de produits* »
— [article 52 §16](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Rétention documentaire liée** : art. 13 §13 — documentation technique et déclaration UE de
conformité tenues à disposition « *pendant une durée d'au moins dix ans après la mise sur le marché
[…] ou pendant la période d'assistance, la période la plus longue étant retenue* »
— [article 13 §13](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

### Inferences

- Trois durées distinctes ne doivent pas être confondues : (a) **période d'assistance** ≥ 5 ans
  (traitement des vulnérabilités) ; (b) **disponibilité au téléchargement** de chaque mise à jour
  déjà publiée ≥ 10 ans après son émission ; (c) **conservation du dossier technique** ≥ 10 ans ou
  période d'assistance, la plus longue.
- Le plancher de 5 ans n'est pas absolu : il cède si la durée d'utilisation prévue est plus courte,
  mais c'est au fabricant de documenter ce raisonnement dans le dossier technique (art. 13 §8
  al. 5), sous contrôle des autorités (art. 52 §16).

### Gaps

- Aucun acte délégué fixant une période d'assistance minimale sectorielle (art. 13 §8 al. 4) n'a été
  identifié — **non vérifié à la source**.

---

## Q6 — Composant logiciel intégré dans le produit d'un autre fabricant ; une bibliothèque est-elle un « produit comportant des éléments numériques » ?

### Takeaway

**Oui** : la définition de l'article 3, point 1 couvre explicitement « *les composants logiciels ou
matériels mis sur le marché séparément* ». Un composant mis sur le marché séparément est donc un
produit comportant des éléments numériques à part entière, et son fournisseur est « fabricant » s'il
le commercialise sous son nom, « *à titre onéreux, monétisé ou gratuit* ». Côté intégrateur, le CRA
impose une **diligence raisonnable** (art. 13 §5) et une obligation de **remonter la vulnérabilité au
mainteneur du composant**, y compris open source, avec partage du correctif (art. 13 §6). Cas
particulier décisif pour l'open source : un composant FOSS destiné à être intégré par d'autres
fabricants n'est « mis à disposition sur le marché » **que s'il est monétisé par son fabricant
d'origine** (considérant 18).

### Cited Findings

**Article 3, point 1 — définition** (texte exact) :

> « “produit comportant des éléments numériques”: un produit logiciel ou matériel et ses solutions de traitement de données à distance, y compris **les composants logiciels ou matériels mis sur le marché séparément**; »
> — [article 3, point 1](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 3, point 6 — « composant »** : « *un logiciel ou du matériel destiné à être intégré dans un
système d'information électronique;* »
— [article 3, point 6](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 3, point 13 — « fabricant »** (texte exact) :

> « une personne physique ou morale qui développe ou fabrique des produits comportant des éléments numériques ou fait concevoir, développer ou fabriquer des produits comportant des éléments numériques, et les commercialise sous son propre nom ou sa propre marque, **à titre onéreux, monétisé ou gratuit**; »
> — [article 3, point 13](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 13 §5 — diligence raisonnable de l'intégrateur** (texte exact) :

> « Aux fins du respect de l'obligation énoncée au paragraphe 1, les fabricants font preuve de diligence raisonnable lorsqu'ils intègrent dans des produits comportant des éléments numériques des composants obtenus auprès de tiers, de sorte que ces composants ne compromettent pas la cybersécurité du produit comportant des éléments numériques, y compris lors de l'intégration de composants de logiciels libres et ouverts qui n'ont pas été mis à disposition sur le marché dans le cadre d'une activité commerciale. »
> — [article 13 §5](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 13 §6 — remontée au mainteneur et partage du correctif** (texte exact) :

> « Lorsqu'ils identifient une vulnérabilité dans un composant, y compris un composant logiciel ouvert, qui est intégré au produit comportant des éléments numériques, les fabricants signalent la vulnérabilité à la personne ou à l'entité qui assure la maintenance du composant, et s'attaquent et remédient à la vulnérabilité conformément aux exigences relatives à la gestion des vulnérabilités énoncées à l'annexe I, partie II. Lorsque les fabricants ont mis au point une modification logicielle ou matérielle pour remédier à la vulnérabilité de ce composant, ils partagent le code ou la documentation correspondants avec la personne ou l'entité qui fabrique le composant ou en assure la maintenance, dans un format lisible par machine s'il y a lieu. »
> — [article 13 §6](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Monétisation du composant : critère de mise sur le marché — considérant 18** (extrait exact) :

> « En outre, la fourniture de produits comportant des éléments numériques qui répondent aux critères de logiciels libres et ouverts, destinés à être intégrés par d'autres fabricants à leurs propres produits comportant des éléments numériques, ne devrait être considérée comme une mise à disposition sur le marché **que si le composant est monétisé par son fabricant d'origine**. »
> — [considérant 18](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Information à fournir à l'intégrateur — annexe II, point 8 f)** (texte exact) :

> « lorsque le produit comportant des éléments numériques est destiné à être intégré dans d'autres produits comportant des éléments numériques, les informations nécessaires pour que l'intégrateur se conforme aux exigences essentielles de cybersécurité énoncées à l'annexe I et aux exigences en matière de documentation énoncées à l'annexe VII. »
> *(traduction de travail à partir du libellé FR du JO : « …les informations nécessaires à l'intégrateur pour respecter les exigences essentielles de cybersécurité énoncées à l'annexe I et les exigences de documentation énoncées à l'annexe VII » — libellé EN verbatim : « where the product with digital elements is intended for integration into other products with digital elements, the information necessary for the integrator to comply with the essential cybersecurity requirements set out in Annex I and the documentation requirements set out in Annex VII »)*
> — [annexe II, point 8 f), version EN](https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=OJ:L_202402847)

**Diligence raisonnable vis-à-vis des composants tiers — considérant 31** (repéré, non cité
intégralement) : le texte indique que, lorsqu'il intègre des composants obtenus auprès de tiers lors
de la conception et du développement, le fabricant « devrait » accomplir certaines vérifications
— [considérant au titre de la diligence sur les composants tiers](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)
(**le numéro exact de ce considérant n'a pas été vérifié à la source**).

**Annexe I, partie II, point 6 — partage d'information sur les composants tiers** : les fabricants
« *prennent des mesures pour faciliter le partage d'informations sur les vulnérabilités potentielles
de leurs produits […] ainsi que des composants tiers contenus dans ces produits* »
— [annexe I, partie II, point 6](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 25 — attestation de sécurité volontaire pour le FOSS** : habilitation de la Commission à
créer « *des programmes volontaires d'attestation de sécurité* » pour faciliter le respect de la
diligence raisonnable de l'article 13 §5, « *en particulier en ce qui concerne les fabricants qui
intègrent des composants logiciels libres et ouverts dans leurs produits* »
— [article 25](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Exclusion des pièces de rechange — article 2 §6** : le règlement ne s'applique pas « *aux pièces de
rechange qui sont mises à disposition sur le marché pour remplacer des composants identiques dans
des produits comportant des éléments numériques et qui sont fabriquées conformément aux mêmes
spécifications que les composants* [qu'elles remplacent] »
— [article 2 §6](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

### Inferences

- Une **bibliothèque logicielle** entre bien dans la notion de « produit comportant des éléments
  numériques » dès lors qu'elle est « mise sur le marché séparément » (art. 3, point 1) — le CRA ne
  réserve pas ce statut aux produits finis.
- Le pivot n'est donc pas « bibliothèque vs produit fini » mais **« mise à disposition sur le marché
  dans le cadre d'une activité commerciale »** ; pour un composant FOSS destiné à l'intégration, le
  considérant 18 ramène ce test à la **monétisation par le fabricant d'origine**.
- Asymétrie importante : même si le composant est hors champ (FOSS non monétisé), l'**intégrateur**
  reste tenu (art. 13 §5 et §6) — le CRA transfère la charge vers l'aval de la chaîne plutôt que de
  l'imposer au projet amont.

### Gaps

- La traduction FR exacte de l'annexe II, point 8 f) n'a pas été extraite verbatim du JO FR (le
  libellé EN est cité verbatim) — à revérifier avant citation dans un document de conformité.
- Le numéro du considérant traitant de la diligence raisonnable sur les composants tiers n'a pas été
  isolé.

---

## Q7 — Open source : définition, intendant de logiciels ouverts, projet MIT individuel non monétisé

### Takeaway

Le CRA ne s'applique **pas** à un logiciel libre qui n'est pas « mis à disposition sur le marché »,
c'est-à-dire fourni « *pour être distribué ou utilisé dans le cadre d'une activité commerciale* » ; et
« *la fourniture de produits […] qui répondent aux critères de logiciels libres et ouverts **qui ne
sont pas monétisés par leur fabricant** ne devrait pas être considérée comme une activité
commerciale* » (considérant 18). Le règlement ne s'applique pas non plus aux personnes qui
contribuent du code à des projets « *ne relevant pas de leur responsabilité* ». L'« **intendant de
logiciels ouverts** » (*open-source software steward*, art. 3 point 14) est une **personne morale**
— jamais une personne physique — soumise à un régime allégé (art. 24), sans marquage CE et **sans
amendes administratives** (art. 64 §10 b)).

### Cited Findings

**Définition — article 3, point 14 (« intendant de logiciels ouverts »)** (texte exact) :

> « une personne morale, autre que le fabricant, qui a pour objectif ou finalité de fournir un soutien systématique et continu au développement de produits spécifiques comportant des éléments numériques qui répondent aux critères de logiciels libres et ouverts et sont destinés à des activités commerciales, et qui assure la viabilité de ces produits; »
> — [article 3, point 14](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 24 — « Obligations des intendants de logiciels ouverts »** (texte exact, intégral) :

> « 1. Les intendants de logiciels ouverts mettent en place et documentent de manière vérifiable une politique de cybersécurité afin de favoriser le développement d'un produit comportant des éléments numériques sécurisé ainsi qu'un traitement efficace des vulnérabilités par les développeurs de ce produit. Cette politique encourage également le signalement volontaire des vulnérabilités, prévu à l'article 15, par les développeurs de ce produit et tient compte de la nature spécifique de l'intendant de logiciels ouverts et des modalités juridiques et organisationnelles auxquelles il est soumis. Cette politique comprend, en particulier, des aspects liés à la documentation, au traitement et à la correction des vulnérabilités, ainsi qu'à la promotion du partage d'informations sur les vulnérabilités découvertes au sein de la communauté des logiciels ouverts.
>
> 2. Les intendants de logiciels ouverts coopèrent avec les autorités de surveillance du marché, à leur demande, en vue d'atténuer les risques de cybersécurité posés par un produit comportant des éléments numériques qui répond aux critères de logiciel libre et ouvert.
> Sur demande motivée d'une autorité de surveillance du marché, les intendants de logiciels ouverts fournissent à cette autorité, dans une langue aisément compréhensible par celle-ci, la documentation visée au paragraphe 1, sur support papier ou sous forme électronique.
>
> 3. Les obligations prévues à l'article 14, paragraphe 1, s'appliquent aux intendants de logiciels ouverts dès lors qu'ils participent au développement des produits comportant des éléments numériques. Les obligations prévues à l'article 14, paragraphes 3 et 8, s'appliquent aux intendants de logiciels ouverts dès lors que des incidents graves ayant des répercussions sur la sécurité des produits comportant des éléments numériques touchent les réseaux et les systèmes d'information fournis par les intendants de logiciels ouverts pour le développement de ces produits. »
> — [article 24](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Considérant 18 — le test de commercialité et le sort des contributeurs individuels** (extraits
exacts, enchaînés) :

> « On entend par “logiciel libre et ouvert” un logiciel dont le code source est partagé de manière ouverte et dont la licence prévoit tous les droits pour qu'il soit librement accessible, utilisable, modifiable et redistribuable. […] En ce qui concerne les opérateurs économiques auxquels s'applique le présent règlement, **seuls les logiciels libres et ouverts mis à disposition sur le marché, donc fournis pour être distribués ou utilisés dans le cadre d'une activité commerciale, devraient relever du champ d'application du présent règlement.** Les seules circonstances dans lesquelles le produit comportant des éléments numériques a été développé ou la manière dont le développement a été financé ne devraient donc pas être prises en considération au moment de déterminer si l'activité en question est de nature commerciale ou non. Plus précisément, […] **la fourniture de produits comportant des éléments numériques qui répondent aux critères de logiciels libres et ouverts qui ne sont pas monétisés par leur fabricant ne devrait pas être considérée comme une activité commerciale.** En outre, la fourniture de produits […] destinés à être intégrés par d'autres fabricants à leurs propres produits […] ne devrait être considérée comme une mise à disposition sur le marché que si le composant est monétisé par son fabricant d'origine. Par exemple, le simple fait qu'un fabricant verse un soutien financier à un logiciel libre comportant des éléments numériques ou qu'il contribue au développement d'un tel produit ne devrait pas en soi suffire à déterminer que cette activité est de nature commerciale. En outre, les mises à jour régulières de ce logiciel ne devraient pas permettre à elles seules de conclure qu'un produit comportant des éléments numériques est fourni dans le cadre d'une activité commerciale. Enfin, […] le développement par des organisations à but non lucratif de produits […] ne devrait pas être considéré comme une activité commerciale, pour autant que l'organisation concernée soit constituée de telle façon que tous les bénéfices sont utilisés pour atteindre des objectifs non lucratifs. **Le présent règlement ne s'applique pas aux personnes physiques ou morales qui contribuent, sous forme de code source, à des produits comportant des éléments numériques qui répondent aux critères de logiciels libres et ouverts ne relevant pas de leur responsabilité.** »
> — [considérant 18](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Considérant 17 — modèles de développement distribués** : « *Pour favoriser le développement et le
déploiement de logiciels libres et ouverts, en particulier par les microentreprises et les petites et
moyennes entreprises, y compris les jeunes pousses, par les personnes physiques, par les
organisations à but non lucratif et par les instituts de recherche universitaires, l'application du
présent règlement aux produits […] qui répondent aux critères de logiciels libres et ouverts fournis
pour être distribués ou utilisés dans le cadre d'une activité commerciale devrait tenir compte de la
nature des différents modèles de développement de logiciels distribués et développés sous licences
logicielles libres et ouvertes.* »
— [considérant 17](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Considérant 19 — qui est intendant, et pas de marquage CE** (extraits exacts) :

> « Étant donné l'importance que revêtent, en matière de cybersécurité, de nombreux produits comportant des éléments numériques qui répondent aux critères de logiciels libres et ouverts qui sont publiés mais ne sont pas mis à disposition sur le marché au sens du présent règlement, les personnes morales qui apportent un soutien prolongé au développement de tels produits destinés à des activités commerciales et qui jouent un rôle de premier plan en veillant à la viabilité de ces produits (intendants de logiciels ouverts) devraient être soumises à un régime réglementaire allégé et sur mesure. **Figurent parmi les intendants de logiciels ouverts certaines fondations et les entités qui développent et publient des logiciels libres et ouverts dans un cadre commercial, y compris les entités à but non lucratif.** […] Ce régime ne devrait concerner que les produits […] dont la finalité est commerciale, par exemple ceux qui sont destinés à être intégrés à des services commerciaux ou à des produits comportant des éléments numériques monétisés. Aux fins de ce régime réglementaire, l'intention d'intégration à des produits comportant des éléments numériques monétisés couvre les cas où le fabricant qui intègre un composant dans ses propres produits […] contribue régulièrement au développement de ce composant ou apporte une assistance financière régulière afin d'assurer la pérennité d'un logiciel. **Le fait d'apporter un soutien prolongé au développement d'un produit comportant des éléments numériques comprend, sans s'y limiter, l'hébergement et la gestion de plates-formes collaboratives de développement de logiciels, l'hébergement de code source ou d'un logiciel, l'administration ou la gestion de produits […] ainsi que le pilotage du développement de ces produits.** Étant donné que le régime réglementaire allégé et sur mesure n'impose pas aux intendants de logiciels ouverts les mêmes obligations que celles qui incombent aux fabricants […], **les intendants de logiciels ouverts ne devraient pas être autorisés à apposer le marquage CE** aux produits comportant des éléments numériques dont ils soutiennent le développement. »
> — [considérant 19](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Considérant 20 — héberger sur un dépôt ouvert ≠ mise sur le marché** (texte exact) :

> « Le seul fait d'héberger des produits comportant des éléments numériques sur des dépôts ouverts, y compris par l'intermédiaire de progiciels ou de plates-formes collaboratives, ne constitue pas en soi la mise à disposition sur le marché d'un produit comportant des éléments numériques. Les fournisseurs de ces services ne devraient être considérés comme des distributeurs que s'ils mettent ces logiciels à disposition sur le marché, donc s'ils les fournissent pour qu'ils soient distribués ou utilisés sur le marché de l'Union dans le cadre d'une activité commerciale. »
> — [considérant 20](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Pas d'amendes pour les intendants — article 64 §10** (texte exact) :

> « Par dérogation aux paragraphes 3 à 9, les amendes administratives visées auxdits paragraphes ne s'appliquent pas:
> a) aux fabricants considérés comme des microentreprises ou des petites entreprises en cas de non-respect du délai visé à l'article 14, paragraphe 2, point a), ou à l'article 14, paragraphe 4, point a);
> b) à toute violation du présent règlement par les intendants de logiciels ouverts. »
> — [article 64 §10](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 29 §5 — procédure allégée pour le FOSS listé en annexe III** : « *Les fabricants de produits
comportant des éléments numériques qui répondent aux critères de logiciels libres et ouverts et
relèvent des catégories énoncées à l'annexe III ont [régime particulier]* »
— [article 29 §5](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)
(**le dispositif exact de ce paragraphe n'a pas été extrait verbatim — non vérifié à la source**)

**Considérant 16 — récupération de coûts réels** : « *les produits comportant des éléments numériques
fournis dans le cadre d'une prestation de service pour laquelle une rétribution est perçue à la
seule fin de récupérer les coûts réels directement liés au fonctionnement de ce service […] ne
devraient pas être considérés pour cette seule raison comme constituant une activité commerciale* »
— [considérant 16](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Confirmation administrative (Commission)** : « Only free and open-source software that is made
available on the market, and therefore supplied for distribution or use in the course of a commercial
activity, falls in scope » ; « The provision of products with digital elements qualifying as free and
open-source software that are not monetised by their manufacturers should not be considered to be a
commercial activity » ; « The CRA does not apply to developers who contribute with source code to
free and open-source software that are not under their responsibility » ; et sur les intendants :
« legal persons who provide support on a sustained basis for the development of such products which
are intended for commercial activities, and who play a main role in ensuring their viability »,
soumis à « a light-touch and tailor-made regulatory regime »
— [Commission européenne, CRA – Open source](https://digital-strategy.ec.europa.eu/en/policies/cra-open-source)

**Orientations pratiques de la Commission** : « On 27 July 2026, the Commission published practical
guidance to help manufacturers, developers and businesses of all sizes meet their obligations »,
« clarifying when certain products fall within the scope of the Cyber Resilience Act, including
remote data processing solutions and free and open source software »
— [Commission européenne, page CRA](https://digital-strategy.ec.europa.eu/en/policies/cyber-resilience-act) ;
[Commission publishes new guidance to support timely Cyber Resilience Act implementation](https://digital-strategy.ec.europa.eu/en/library/commission-publishes-new-guidance-support-timely-cyber-resilience-act-implementation)

### Inferences

**Cas posé : projet sous licence MIT, développé par une personne physique, non monétisé.**
Sur la base du texte cité :
1. La **licence MIT** est sans incidence directe : le CRA ne liste aucune licence. Le critère du
   considérant 18 est fonctionnel (code source partagé ouvertement + licence conférant les droits
   d'accès, d'usage, de modification et de redistribution) — une licence MIT y répond.
2. Le critère déterminant est la **mise à disposition sur le marché dans le cadre d'une activité
   commerciale**. Non monétisé par son auteur ⇒ selon le considérant 18, « ne devrait pas être
   considéré comme une activité commerciale » ⇒ **hors du champ des obligations de fabricant**.
3. Le **simple hébergement sur un dépôt ouvert** (GitHub, registre de paquets) ne constitue pas une
   mise à disposition sur le marché (considérant 20).
4. Le statut d'**intendant de logiciels ouverts** est exclu : l'article 3, point 14 exige une
   « personne morale ». Une personne physique ne peut pas être intendant.
5. Il ne devient pas fabricant par le seul fait que d'autres intègrent sa bibliothèque : le
   considérant 18 subordonne ce basculement à la **monétisation par le fabricant d'origine**.
6. Points de vigilance (déductions, non des règles explicites) : la monétisation indirecte (support
   payant, double licence, édition « pro », sponsoring conditionné à des prestations) et la mise sur
   le marché sous son propre nom d'un produit construit autour du projet pourraient faire basculer
   l'analyse ; ces hypothèses ne sont pas tranchées dans le texte.
7. Les considérants ne sont pas normatifs en eux-mêmes, mais ici le dispositif (art. 3 point 13
   « commercialise sous son propre nom […] à titre onéreux, monétisé ou gratuit », art. 2 champ
   d'application, art. 13 §5) est interprété par les considérants 16 à 20, et la Commission reprend
   cette lecture mot pour mot sur sa page officielle, ce qui la rend robuste.

### Gaps

- La notion de « monétisé » n'est **pas définie** dans le règlement (aucune définition à l'article 3).
  Les orientations de la Commission du 27 juillet 2026 abordent le champ d'application pour le FOSS,
  mais le **contenu détaillé du document d'orientation n'a pas été lu dans cette recherche** (seuls
  les résumés des pages officielles l'ont été) — **non vérifié à la source**.
- Le dispositif exact de l'article 29 §5 (procédure d'évaluation allégée pour le FOSS en annexe III)
  n'a pas été extrait.
- Les articles 2 §1 et §2 (champ d'application positif, exclusion des produits SaaS hors « solutions
  de traitement de données à distance ») n'ont pas été cités verbatim ici.

---

## Q8 — Normes harmonisées : publiées ou en préparation ?

### Takeaway

La demande de normalisation **M/606** (décision d'exécution de la Commission **C(2025)618**) porte
sur « **un ensemble de 41 normes** » horizontales et verticales. À la date du 7 octobre 2026, **aucune
publication de référence de norme harmonisée CRA au *Journal officiel* n'a pu être vérifiée à la
source** : la page officielle de la Commission sur la normalisation CRA ne fournit aucune liste de
références publiées au JO. En l'absence de normes harmonisées, l'article 27 permet à la Commission
d'adopter des **spécifications communes** par actes d'exécution.

### Cited Findings

**Source officielle** : « The European Commission adopted standardisation request **M/606**,
documented as **C(2025)618** in the Register of Commission Documents » ; la demande contient « **a set
of 41 standards in support of the CRA** » ; distinction « horizontal standards » (« provide a common
framework, promoting coherence and offer horizontal processes for compliance with the CRA ») /
« vertical standards » (« product-specific ») ; priorisation des « important and critical product
categories » des annexes III et IV ; livrables de support en cours : « terminology, sectoral risk
assessment methodology, and a common threat catalogue »
— [Commission européenne, CRA – Standardisation](https://digital-strategy.ec.europa.eu/en/policies/cra-standardisation)

La même page officielle **ne fournit aucune information** sur des échéances de livraison ni sur des
références déjà publiées au JO (constat de la consultation du 7 octobre 2026)
— [Commission européenne, CRA – Standardisation](https://digital-strategy.ec.europa.eu/en/policies/cra-standardisation)

**Article 27 §1-2 — présomption de conformité et spécifications communes** : le recours aux
spécifications communes par acte d'exécution n'est possible que si, cumulativement,
« *la Commission […] a demandé à une ou plusieurs organisations européennes de normalisation
d'élaborer une norme harmonisée relative aux exigences essentielles de cybersécurité énoncées à
l'annexe I et: i) la demande n'a pas été acceptée; ii) les normes harmonisées répondant à cette
demande ne sont pas présentées dans le délai fixé […]; ou iii) les normes harmonisées ne sont pas
conformes à la demande* » **et** « *aucune référence à des normes harmonisées couvrant les exigences
essentielles pertinentes de cybersécurité énoncées à l'annexe I […] n'a été publiée au Journal
officiel de l'Union européenne […] et il n'est pas prévu que la publication d'une telle référence
soit publiée dans un délai raisonnable* »
— [article 27 §2](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 27 §5-6** : conformité aux spécifications communes ⇒ présomption de conformité ; et
« *Lorsque la référence d'une norme harmonisée est publiée au Journal officiel […], la Commission
abroge les actes d'exécution* » correspondants
— [article 27 §5 et §6](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Article 27 §8-9** : présomption de conformité par certificat de cybersécurité européen délivré au
titre du règlement (UE) 2019/881, dans la mesure couverte ; habilitation à préciser par acte délégué
quels schémas peuvent être utilisés
— [article 27 §8 et §9](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Annexe VII, point 5** : la documentation technique doit lister « *les normes harmonisées, appliquées
entièrement ou en partie, dont les références ont été publiées au Journal officiel […], des
spécifications communes telles que définies à l'article 27 […] ou des schémas européens de
certification de cybersécurité* » et, à défaut, « *une présentation des solutions adoptées pour
répondre aux exigences essentielles de cybersécurité énoncées à l'annexe I, parties I et II, y
compris une liste des autres spécifications techniques pertinentes appliquées* »
— [annexe VII, point 5](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Définition — article 3** : « *“norme harmonisée”: une norme harmonisée au sens de l'article 2,
point 1) c), du règlement (UE) n° 1025/2012;* »
— [article 3](https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=OJ:L_202402847)

**Informations non primaires, à traiter avec prudence** — une recherche web a renvoyé des éléments
d'avancement convergents mais issus de **cabinets de conseil et de sites spécialisés, pas de sources
officielles**, et qui n'ont **pas** pu être confirmés à la source :
- acceptation de M/606 par CEN, CENELEC et ETSI pour ~41 normes (15 horizontales) ; échéance initiale
  des normes de type A et des normes de type B « gestion des vulnérabilités » fixée au 30 août 2026 ;
  projet d'amendement publié début juillet 2026 reportant de deux mois les échéances 2026 (type A et
  B au 31 octobre 2026, normes C au 31 décembre 2026) — [IBF Solutions](https://www.ibf-solutions.com/en/news-and-knowledge/technical-papers-and-news-on-ce-marking/current-status-of-standardisation-for-the-cyber-resilience-act)
- **EN 40000-1-1** (vocabulaire) et **EN 40000-1-2** (principes, gestion des risques produit et
  activités de cycle de vie) auraient passé le vote formel CEN-CENELEC, ratification programmée au
  2 novembre 2026 ; EN 40000-1-3 (gestion des vulnérabilités), prEN 40000-10, prEN 40000-11,
  prEN 40001-3, prEN 40002-3, prEN 40003-3 et prEN 18330 en cours d'approbation — [IBF Solutions](https://www.ibf-solutions.com/en/news-and-knowledge/technical-papers-and-news-on-ce-marking/current-status-of-standardisation-for-the-cyber-resilience-act) ; repris par [cyberresilienceact.eu – State of Play](https://www.cyberresilienceact.eu/state-of-play.html) et des trackers commerciaux ([craevidence](https://craevidence.com/cra-compliance/harmonised-standards-status), [cenitia](https://cenitia.com/library/cra-harmonised-standards-tracker))
- ⚠️ Ces sources indiquent qu'**aucune référence n'est encore citée au *Journal officiel***, ce qui est
  cohérent avec l'absence de liste sur la page officielle de la Commission, mais reste **non vérifié
  à la source**.

### Inferences

- Tant qu'aucune référence de norme harmonisée n'est citée au JO, la présomption de conformité de
  l'article 27 §1 est **indisponible** : un fabricant doit documenter ses propres solutions
  techniques au titre de l'annexe VII, point 5 (dernière branche). Ce n'est pas un blocage juridique
  — les exigences essentielles ne sont pas applicables avant le 11 décembre 2027 — mais un risque de
  calendrier : la ratification d'une EN n'équivaut pas à la citation de sa référence au JO, seule
  étape qui ouvre la présomption de conformité.
- La numérotation « EN 40000-x » est cohérente avec une série horizontale CEN/CENELEC dédiée, mais la
  liste ci-dessus doit être **revérifiée sur les catalogues CEN/CENELEC et au JO** avant toute
  citation dans un document de conformité.

### Gaps

- **Non vérifié à la source** : l'existence, la date et le contenu exact de M/606 n'ont pas été lus
  dans le document C(2025)618 lui-même (seule la page officielle de la Commission qui le référence a
  été consultée) ; aucune décision d'exécution n'a été récupérée depuis EUR-Lex ou le registre des
  documents de la Commission.
- **Non vérifié à la source** : l'état d'avancement normatif (EN 40000-1-1 / -1-2, échéances
  reportées) provient exclusivement de sources secondaires non officielles.
- **Non vérifié à la source** : aucune recherche n'a été faite dans la base « Harmonised Standards »
  de la Commission ni au JO série L/C pour confirmer l'absence de citation de référence CRA au
  7 octobre 2026.
- Aucune source ENISA n'a été consultée dans cette recherche (l'assignation la positionnait en
  complément facultatif) : les éventuels guides techniques ENISA sur le CRA sont donc absents de ces
  notes.

---

## Annexe — récapitulatif des ancrages normatifs (pour rédaction)

| Sujet | Référence exacte | Applicable au 2026-10-07 ? |
|---|---|---|
| Exigences essentielles produit | annexe I, partie I, points 1 et 2 a)–m) | Non — 11.12.2027 |
| Gestion des vulnérabilités | annexe I, partie II, points 1)–8) | Non — 11.12.2027 |
| SBOM | annexe I, partie II, point 1) ; art. 13 §24 (format) ; annexe VII, points 2 b) et 8 ; annexe II, point 9 ; cons. 77 | Non — 11.12.2027 |
| Période d'assistance ≥ 5 ans | art. 13 §8 ; art. 3 (déf.) ; annexe II, point 7 ; art. 13 §19 | Non — 11.12.2027 |
| Mises à jour disponibles ≥ 10 ans | art. 13 §9 | Non — 11.12.2027 |
| Signalement vuln. activement exploitée (24 h / 72 h / 14 j) | art. 14 §1 et §2 | **Oui — depuis 11.09.2026** |
| Signalement incident grave (24 h / 72 h / 1 mois) | art. 14 §3, §4, §5 | **Oui — depuis 11.09.2026** |
| Information des utilisateurs | art. 14 §8 | **Oui — depuis 11.09.2026** |
| Plateforme unique de signalement | art. 16 ; art. 14 §1 et §7 | **Oui — opérationnelle depuis 11.09.2026** (source Commission) |
| Signalement volontaire | art. 15 | **Oui — depuis 11.09.2026** (via l'art. 14 ; à confirmer pour l'art. 15 lui-même) |
| Composant mis sur le marché séparément | art. 3 point 1 ; art. 3 point 6 | Non — 11.12.2027 |
| Diligence sur composants tiers / FOSS | art. 13 §5 et §6 ; annexe I partie II point 6 | Non — 11.12.2027 |
| Intendant de logiciels ouverts | art. 3 point 14 ; art. 24 ; cons. 19 ; art. 64 §10 b) | Non — 11.12.2027 |
| FOSS non monétisé hors champ | cons. 17, 18, 20 | s.o. (règle de champ) |
| Normes harmonisées / spéc. communes | art. 27 ; annexe VII point 5 ; M/606 = C(2025)618 | Aucune référence au JO vérifiée |
| Transitoire parc installé | art. 69 §2 et §3 (divergence FR/EN sur §3) | — |
| Organismes notifiés | chapitre IV, art. 35–51 | **Oui — depuis 11.06.2026** |

⚠️ La ligne « Signalement volontaire » mérite vérification : l'article 71 §2 ne cite nommément que
l'article 14 et le chapitre IV comme applicables avant le 11 décembre 2027 ; l'article 15 n'y figure
pas, ce qui suggère qu'il n'est applicable qu'au 11 décembre 2027 — **non vérifié à la source** et à
trancher avant usage.
