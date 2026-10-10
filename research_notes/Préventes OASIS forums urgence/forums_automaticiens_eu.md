# Forums automaticiens FR / DE / NL — besoins acheteurs 2026 autour de 2023/1230 Annexe III 1.1.9 / 1.2.1(f), EN 50742, authentification des commandes PLC, journal inaltérable

**Date de recherche : 2026-10-10.** Méthode : ~30 requêtes web (dont requêtes restreintes à sps-forum.de, support.industry.siemens.com, community.se.com, forge.codesys.com, infosys.beckhoff.com, plcforum.fr, forums.automation.fr, automation-sense.com, plc-forum.nl, linkedin.com). 

**Limite majeure (à lire avant tout) :** le bac à sable de recherche a refusé toute connexion directe (proxy 403 CONNECT) vers sps-forum.de, support.industry.siemens.com, forge.codesys.com, dguv.de, apave.com, eurogip.fr, d-sc.nl, industrievandaag.nl et web.archive.org ; reddit.com, xing.com et tweakers.net sont bloqués pour le robot. **Aucun fil de forum n'a donc pu être lu intégralement** : les citations ci‑dessous proviennent des extraits renvoyés par le moteur de recherche, pas de la page d'origine. Les dates, auteurs, rôles et entreprises des posteurs **n'ont pas pu être relevés**. Le budget de recherche web partagé a été épuisé avant la fin (deux requêtes finales sur plcforum.fr/Modbus non exécutées). Toutes les probabilités de pré‑vente ci‑dessous sont donc des estimations basses, à re‑vérifier par lecture directe des fils depuis un poste non filtré.

---

## Question 1 — Quels fils (sps-forum.de, Siemens, Schneider, Beckhoff, forums FR, Codesys, NL) posent en 2026 la question du 1.1.9 / cybersécurité du Règlement Machines ou de l'EN 50742 ?

### Takeaway
Seul **sps-forum.de** présente des fils identifiables sur la cybersécurité du Règlement Machines, et un seul d'entre eux est manifestement de 2026 (TIA V21). Aucun fil 2026 n'a été trouvé sur le forum Siemens Industry Online Support, community.se.com, Beckhoff Infosys, CODESYS Forge, plcforum.fr / forums.automation.fr / automation-sense.com, ni sur les forums NL (plc-forum.nl, circuitsonline). Le vide est en soi un signal : la demande « 1.1.9 » s'exprime pour l'instant via webinaires, formations et articles de fournisseurs, pas dans les forums d'automaticiens.

### Cited Findings — fils identifiés (classés par probabilité estimée de pré‑vente sous 30 jours)

#### F1 — sps-forum.de : « TIA V21 – Secure PG/PC- und HMI-Kommunikation deaktivieren » (ID 121228)
- URL : https://www.sps-forum.de/threads/tia-v21-secure-pg-pc-und-hmi-kommunikation-deaktivieren.121228/ — [sps-forum.de](https://www.sps-forum.de/threads/tia-v21-secure-pg-pc-und-hmi-kommunikation-deaktivieren.121228/)
- Date : **non lue** ; l'ID 121228 et le sujet (TIA Portal V21, S7-1200 G2) situent le fil en 2026 (un fil voisin ID 120919 porte aussi sur TIA V21 / MTP1000) — [sps-forum.de](https://www.sps-forum.de/threads/gel%C3%B6st-tia-v21-mtp1000-warnzeichen-auf-schaltfl%C3%A4chen-und-e-a-feldern.120919/)
- Forum / langue : sps-forum.de (DE). Auteur / rôle / pays : **non visibles dans l'extrait**.
- Contenu (paraphrase d'extrait) : un utilisateur veut **désactiver** la case « Secure PG/PC- und HMI-Kommunikation » dans TIA V21 (case grisée, possible en V20) ; une réponse indique que la case reste actionnable en V21 Update 1 ; un autre soupçonne que c'est impossible sur S7‑1200 G2 ; un intervenant rappelle que les anciens panels (<V17) ne peuvent pas établir de connexion sécurisée — [sps-forum.de](https://www.sps-forum.de/threads/tia-v21-secure-pg-pc-und-hmi-kommunikation-deaktivieren.121228/)
- Citation (DE, extrait moteur, non vérifiée) : « *EU cybersecurity rules apply to machine building and unencrypted connections should no longer be used* » (restitué en anglais par le moteur ; libellé allemand d'origine non récupéré) — [sps-forum.de](https://www.sps-forum.de/threads/tia-v21-secure-pg-pc-und-hmi-kommunikation-deaktivieren.121228/)
  - Traduction FR : « Les règles cyber de l'UE s'appliquent à la construction de machines et les connexions non chiffrées ne devraient plus être utilisées. »
- Urgence : aucune échéance citée dans l'extrait. **Attention** : le demandeur cherche à *réduire* la sécurité (compatibilité panels anciens), ce n'est pas un acheteur de gateway. L'intérêt est la pression réglementaire exprimée par les répondants.
- Ce qu'il a essayé : désactivation de l'option dans TIA V21 (grisée).
- Probabilité pré‑vente 30 j : **faible (≈ 5 %)**.
- Réponse FR proposée (3 lignes) :
  1. « Désactiver la com sécurisée pour garder vos anciens panels vous remet dans le cas que le 1.1.9 (2023/1230) vise à partir du 20/01/2027 : un canal non authentifié vers l'automate. »
  2. « Alternative sans toucher aux panels : une passerelle qui n'accepte que des commandes signées et journalise chaque écriture (qui, quand, quelle valeur) de façon inaltérable 5 ans. »
  3. « Si vous voulez, je décris en MP comment on l'a câblé devant un S7 en 1 journée, sans modifier le programme TIA. »

#### F2 — sps-forum.de : « NIS2 (!?) End-to-end Kryptierung wird gefordert. » (ID 112995)
- URL : https://www.sps-forum.de/threads/nis2-end-to-end-kryptierung-wird-gefordert.112995/ — [sps-forum.de](https://www.sps-forum.de/threads/nis2-end-to-end-kryptierung-wird-gefordert.112995/)
- Date : **ancien** — le moteur indique « ~1 000 jours » (≈ fin 2023 / début 2024) et que « l'implémentation allemande [NIS2] n'était pas encore disponible » — [sps-forum.de](https://www.sps-forum.de/threads/nis2-end-to-end-kryptierung-wird-gefordert.112995/). À signaler **séparément comme fil ancien** ; activité 2026 non vérifiable.
- Forum / langue : sps-forum.de (DE). Auteur / rôle : non visibles.
- Besoin exprimé : un client exige un chiffrement de bout en bout au titre de NIS2 ; débat sur le périmètre.
- Citations (DE, extraits moteur) :
  - « *Die MRL fordert "nur" die Security der Sicherheitssteuerung bzw. der Sicherheitsfunktionen (Not-Halt, Schutztür etc.).* » — [sps-forum.de](https://www.sps-forum.de/threads/nis2-end-to-end-kryptierung-wird-gefordert.112995/)
    - FR : « Le Règlement Machines n'exige "que" la sécurité de la commande de sécurité, c.-à-d. des fonctions de sécurité (arrêt d'urgence, porte de protection, etc.). »
  - Contre‑avis (paraphrase moteur) : « on ne s'en sortira pas avec un mot de passe et un module pare‑feu » ; un autre s'attend à ce que les constructeurs ajoutent des pare‑feu dans les armoires — [sps-forum.de](https://www.sps-forum.de/threads/nis2-end-to-end-kryptierung-wird-gefordert.112995/)
- Urgence : exigence client explicite, mais ancienne ; pas d'échéance datée dans l'extrait.
- Ce qu'il a essayé : non visible.
- Probabilité pré‑vente 30 j : **faible (≈ 5 %)** sauf si le fil a été réactivé en 2026 (non vérifié).
- Réponse FR proposée :
  1. « Deux ans après ce fil, le débat est tranché par le texte : 1.1.9 + 1.2.1(f) imposent de protéger la commande contre la corruption ET de conserver 5 ans le journal des interventions logicielles. »
  2. « Le chiffrement seul ne répond pas au 1.2.1(f) ; il faut l'authentification de chaque ordre et un journal que ni l'opérateur ni l'intégrateur ne peuvent effacer. »
  3. « On a une passerelle qui fait exactement ces deux choses devant un automate existant ; je peux poster le schéma si ça intéresse. »

#### F3 — sps-forum.de : « SBOM von SPS-Herstellern » (ID 114013)
- URL : https://www.sps-forum.de/threads/sbom-von-sps-herstellern.114013/ — [sps-forum.de](https://www.sps-forum.de/threads/sbom-von-sps-herstellern.114013/)
- Date : non lue ; l'ID (114013, voisin de 114005 daté ≈ 2024 par le moteur) suggère **2024**, donc à classer « ancien ». 
- Besoin : un constructeur se demande s'il doit produire sa propre SBOM ou seulement celles de ses fournisseurs sous le CRA ; renvoi vers BSI TR‑03183 — [sps-forum.de](https://www.sps-forum.de/threads/sbom-von-sps-herstellern.114013/)
- Pertinence OASIS : indirecte (CRA, pas commande/journal). Probabilité : **très faible (< 3 %)**.
- Réponse FR : (1) « La SBOM couvre le CRA ; pour le Règlement Machines c'est le journal 5 ans des versions et interventions (1.2.1(f)) qui manque le plus souvent. » (2) « Les deux se rejoignent si la passerelle qui autorise les écritures enregistre aussi la version logicielle chargée. » (3) « Je partage volontiers notre matrice CRA × 2023/1230 par exigence. »

#### F4 — sps-forum.de : « Software als Sicherheitsbauteil – Ein Blick in die Maschinenverordnung » (ID 118964)
- URL : https://www.sps-forum.de/threads/software-als-sicherheitsbauteil-ein-blick-in-die-maschinenverordnung.118964/ — [sps-forum.de](https://www.sps-forum.de/threads/software-als-sicherheitsbauteil-ein-blick-in-die-maschinenverordnung.118964/)
- Nature : **billet fournisseur (Pilz)**, « aperçu du point de vue du fabricant », pas une question d'acheteur — [sps-forum.de](https://www.sps-forum.de/threads/software-als-sicherheitsbauteil-ein-blick-in-die-maschinenverordnung.118964/). Date non lue (ID ≈ 2025). Réponses éventuelles d'utilisateurs non visibles.
- Probabilité : **nulle comme lead direct** ; utile comme fil où poster une réponse technique visible.

#### F5 — sps-forum.de : « Harmonisierte Normen freier Zugang? Kleine Aktualisierung » (ID 119659)
- URL : https://www.sps-forum.de/threads/harmonisierte-normen-freier-zugang-kleine-aktualisierung.119659/ — [sps-forum.de](https://www.sps-forum.de/threads/harmonisierte-normen-freier-zugang-kleine-aktualisierung.119659/)
- Contenu (extrait) : un post « d'il y a environ un an » constate qu'« *aktuell dann auch keine einzige harmonisierte Norm unter der MVO* » (aucune norme harmonisée sous le Règlement Machines) — [sps-forum.de](https://www.sps-forum.de/threads/harmonisierte-normen-freier-zugang-kleine-aktualisierung.119659/). Sujet : accès gratuit aux normes. Pas un lead. Probabilité : **nulle**.

#### F6 — sps-forum.de : « Kommentierter Vergleich Maschinenrichtlinie – Maschinenverordnung » (ID 114005)
- URL : https://www.sps-forum.de/threads/kommentierter-vergleich-maschinenrichtlinie-maschinenverordnung.114005/ — [sps-forum.de](https://www.sps-forum.de/threads/kommentierter-vergleich-maschinenrichtlinie-maschinenverordnung.114005/)
- Contenu (extraits) : rappel des dates de bascule (« *bis zum 19.01.2027 nach 2006/42/EG, ab dem 20.01.2027 nach 2023/1230* », « pas de période de transition, une date butoir ») — [sps-forum.de](https://www.sps-forum.de/threads/kommentierter-vergleich-maschinenrichtlinie-maschinenverordnung.114005/). Fil documentaire, ≈ 2024. Pas de besoin d'achat.

#### F7 — CODESYS Forge : « Audit trail / Audit log » (Visualization)
- URL : https://forge.codesys.com/forge/talk/Visualization/thread/04a3119253/ — [CODESYS Forge](https://forge.codesys.com/forge/talk/Visualization/thread/04a3119253/)
- Date : **2017** selon le moteur ; un utilisateur demandait si CODESYS a un audit log intégré — [CODESYS Forge](https://forge.codesys.com/forge/talk/Visualization/thread/04a3119253/). Aucune activité 2026 détectée. Fil ancien, à noter séparément.

#### Forums sans fil pertinent trouvé (2026)
- **Siemens Industry Online Support** : les requêtes restreintes au domaine ne renvoient que des manuels (Syslog rémanent S7‑1500, F‑Gesamtsignatur du Safety Administration Editor) et un fil de dépannage « Inkonsistenz des Safety-Programms, CPU 1515F-2PN » (n° 198019) sans lien conformité — [Siemens forum](https://support.industry.siemens.com/forum/ww/de/posts/inkonsistenz-des-safety-programms-cpu-1515f-2pn/198019?page=0&pageSize=10)
- **community.se.com (Schneider)** : aucun résultat sur « machinery regulation audit trail ».
- **Beckhoff** : uniquement communiqués de presse d'avril et mai 2026 (« conformité complète des composants safety au Règlement Machines prévue janvier 2027 », certification IEC 62443‑4‑1 attendue en 2026, avis CSAF) et doc Infosys sur l'Audit Trail TwinCAT HMI (licence TF2400, 250 symboles, e‑signature) — [Beckhoff](https://www.beckhoff.com/en-us/company/press/growing-security-requirements-driven-by-the-cyber-resilience-act-and-the-machinery-regulation-2026-04.html) ; [Beckhoff Infosys](https://infosys.beckhoff.com/content/1033/te2000_tc3_hmi_engineering/16201202059.html)
- **automation-sense.com** : seulement des articles 2016‑2019 (ISAGCA, IEC 62443, certification ANSSI S7‑1500) — [automation-sense.com](https://www.automation-sense.com/blog/automatisme/la-cybersecurite-des-automatismes-industriels.html). plcforum.fr et forums.automation.fr : zéro résultat (requête restreinte).
- **NL** : aucun fil forum ; uniquement presse (industrievandaag.nl, engineersonline.nl, vraagenaanbod.nl) et un cabinet (d-sc.nl, classeur Excel gratuit d'évaluation du risque cyber basé sur prEN 50742:2025) — [d-sc.nl](https://d-sc.nl/cyberrisicobeoordeling-voor-machines/)

### Inferences
- La demande « 1.1.9 / journal 5 ans » est aujourd'hui portée par les prescripteurs (Pilz, Apave, exida, IBF, TÜV, consultants) et non par des automaticiens qui posteraient un besoin opérationnel sur un forum : le marché est en phase « sensibilisation », pas « achat de composant ». Un fil forum avec besoin urgent et chiffré n'existe probablement pas encore à cette date.
- Les fils sps-forum existants montrent que les automaticiens allemands pensent « sécurité = firewall / mot de passe / chiffrement », et débattent du périmètre (fonctions de sécurité seulement ?). Le concept « autorisation par commande + journal inaltérable » n'apparaît dans aucun extrait : c'est un angle de différenciation, mais aussi un effort pédagogique à prévoir.

### Gaps
- Dates, auteurs, rôles, entreprises, pays des fils sps-forum : non relevés (hôte bloqué). Impossible de confirmer si F2/F3 ont été réactivés en 2026.
- forums.automation.fr, plcforum.fr, plc-forum.nl, oscat, pdm-forum : aucune page indexée trouvée pour ces termes ; absence ≠ preuve d'absence (indexation faible).
- Siemens forum : pas de recherche interne possible (site bloqué) ; seul l'index Google a été consulté.

---

## Question 2 — Quels fils posent les questions « Modbus Authentifizierung », « signierte Befehle », « manipulationssicheres Logbuch », « Ereignisprotokoll 5 Jahre », « journal des événements règlement machines », « cybersécurité règlement machines 2027 », « benannte Stelle Cybersecurity Maschinenverordnung » ?

### Takeaway
Aucun fil de forum 2026 ne contient littéralement ces expressions. Les fils Modbus trouvés sur sps-forum sont des dépannages de mise en service ; une seule discussion (S7‑1500 ↔ supervision) aborde la limitation des écritures, par restriction des registres exposés, sans authentification.

### Cited Findings
- sps-forum « S7-1500 Automatisierungssystem mit Modbus-TCP Schnittstelle zur übergeordneten Leittechnik » (ID 111973, ≈ 2023‑2024) : conseil de n'exposer que les registres nécessaires — « *Der Modbus-Partner kann nicht wahlfrei in den Daten lesen und schreiben, sondern nur auf die Daten zugreifen, die im DB für MB_HOLD_REG vorhanden sind* » (FR : « Le partenaire Modbus ne peut pas lire/écrire librement, il n'accède qu'aux données présentes dans le DB de MB_HOLD_REG ») — [sps-forum.de](https://www.sps-forum.de/threads/s7-1500-automatisierungssystem-mit-modbus-tcp-schnittstelle-zur-%C3%BCbergeordneten-leittechnik.111973/)
- Autres fils Modbus sps-forum trouvés = dépannage pur : « Modbus- lesen geht, schreiben nicht » (ID 116564), « Modbus als Programmierschnittstelle » (117810), « Modbusregister vom Modbus Server erstellen » (116190), « Modbus Variable lesen und beschreiben » (105084) — [sps-forum.de](https://www.sps-forum.de/threads/modbus-lesen-geht-schreiben-nicht.116564/) ; [sps-forum.de](https://www.sps-forum.de/threads/modbus-als-programmierschnittstelle.117810/)
- Un fournisseur (Trout) recommande un gateway « protocol‑aware » devant l'automate autorisant la lecture aux opérateurs et l'écriture aux seuls ingénieurs nommés, l'identité n'étant pas dans Modbus — [trout.software](https://www.trout.software/resources/securing-modbus) (source commerciale, concurrent potentiel).
- Modbus/TCP Security (TLS) publié en 2018 ; Shodan > 38 000 appareils port 502 exposés — [informatik-aktuell.de](https://www.informatik-aktuell.de/betrieb/sicherheit/modbus-angriffe-im-lokalen-netzwerk.html)
- « Benannte Stelle Cybersecurity Maschinenverordnung » : aucun fil ; la seule mention institutionnelle est la FAQ DGUV Test citant 1.2.1(f) : « *das Rückverfolgungsprotokoll der Daten, das im Zusammenhang mit einem Eingreifen generiert wurden, und der Versionen der Sicherheitssoftware, die nach dem Inverkehrbringen oder der Inbetriebnahme der Maschine oder des dazugehörigen Produkts hochgeladen wurden, bis zu fünf Jahre nach dem Hochladen* » — [DGUV FAQ](https://www.dguv.de/dguv-test/prod-pruef-zert/konform-prod/maschinen/eu-maschinenverordnung/faq-zur-eu-maschinenverordnung/index.jsp)
  - FR : « le journal de traçabilité des données générées lors d'une intervention, et des versions du logiciel de sécurité chargées après la mise sur le marché ou la mise en service de la machine, jusqu'à cinq ans après le chargement ».
- Côté FR, Apave (eWorkshop replay 19/02/2025) : « *exige la traçabilité des modifications logicielles pendant 5 ans, mais ne prescrit pas la méthode technique pour garantir l'intégrité de ce journal* » et « *Le journal de suivi des modifications, pour garantir son intégrité devrait être verrouillé en modification et suppression par les util[isateurs]* » — [Apave](https://france.apave.com/Actualites/eWorkshops/Cybersecurite-et-reglement-machines-19022025)

### Inferences
- Le marché n'a pas encore de vocabulaire pour « commande signée » ; la requête la plus proche est « Modbus Schreibzugriff einschränken ». Le vendeur gagnera à se positionner sur les fils Modbus de dépannage avec le vocabulaire des acheteurs (Schreibschutz, Freigabe, Protokoll) plutôt qu'avec « signature Ed25519 ».
- La phrase Apave « ne prescrit pas la méthode… intégrité du journal » est exactement le trou qu'OASIS comble ; c'est le meilleur « hook » FR trouvé, mais c'est un contenu de prescripteur, pas une question d'acheteur.

### Gaps
- Aucune occurrence 2026 de « Manipulationssicheres Logbuch », « Ereignisprotokoll 5 Jahre », « signierte Befehle » sur les forums ciblés. Impossible de dire si c'est l'absence d'indexation ou l'absence de discussion.
- Les deux requêtes Modbus (EN/FR) prévues en fin de session n'ont pas été exécutées (budget épuisé).

---

## Question 3 — Posts LinkedIn / fils XING de PME Maschinenbau cherchant un fournisseur ?

### Takeaway
Aucun post LinkedIn ou XING de PME **demandant un fournisseur** n'a été trouvé. Le moteur renvoie des profils (conseil, juristes, Gühring, Hänchen…) et des posts d'experts qui *expliquent* le 1.1.9, dont un (DINA) qui pose la question de la re‑certification à chaque mise à jour de sécurité. XING est inaccessible au robot.

### Cited Findings
- Posts LinkedIn (extraits non attribuables à une URL de post ; seules des URLs de profils ont été renvoyées) : « prEN 50742 … still at draft stage, and after a high volume of comments its final publication date is open » ; « manufacturers must demonstrate state of the art themselves » ; un post DINA demande si chaque mise à jour de sécurité terrain impose une re‑certification coûteuse des fonctions de sécurité (« functional safety requires maximum stability, traceability and constant re-validation for software changes ») — exemples de profils renvoyés : [LinkedIn – Anna Flor, KUNZ.law](https://www.linkedin.com/in/anna-flor-8a3a38bb/) ; [LinkedIn – Neil Frost, CERTAIN GmbH](https://www.linkedin.com/in/neil-frost-b867304b/) ; [LinkedIn – Markus Schwenk, Gühring KG](https://www.linkedin.com/in/markus-schwenk-184902245/) ; [LinkedIn – Adam Kuta, Herbert Hänchen GmbH](https://www.linkedin.com/in/adam-kuta-51030761/)
- Événement à exploiter : workshop « Security im Rahmen der neuen EU-Maschinenverordnung », Maschinenbautage Köln, **16 octobre 2026**, intervenant Hans Wilhelm Höfken (yet Industrial IT Security GmbH) — [maschinenrichtlinie.de](https://www.maschinenrichtlinie.de/fortbildung/konferenzen/workshops-maschinenbautage-2026/security-im-rahmen-der-neuen-eu-maschinenverordnung/)
- Formation exida « EN 50742 in Practice » et séminaire IBF « Schutz gegen Korrumpierung nach Maschinenverordnung – die neue EN 50742 » : signaux que les constructeurs achètent de la formation, pas encore des composants — [exida](https://www.exida-eu.com/all-trainings/cybersecurity/457-cybersecurity-en50742-de0613) ; [IBF](https://www.ibf-solutions.com/seminare-und-wissen/technische-spezialseminare/web-seminar-schutz-gegen-korrumpierung-nach-maschinenverordnung-die-neue-en-50742)

### Inferences
- Les posteurs LinkedIn identifiés sont des prescripteurs (cabinets, organismes) : cibles de partenariat plus que d'achat. Les profils d'industriels renvoyés (Gühring, Hänchen) ont « interagi » avec le sujet mais rien ne prouve un besoin exprimé.
- Le Yadulink MCP disponible dans la session pourrait servir à relancer ces profils, mais aucune action d'outreach n'a été faite (hors périmètre, et le brief interdit tout code/action).

### Gaps
- URLs de posts LinkedIn : non obtenues (le moteur ne renvoie que des profils). XING, Reddit : bloqués.
- Aucune preuve d'une PME demandant explicitement « wer bietet… » / « qui propose… ».

---

## Question 4 — Pages Q&A TÜV / DGUV / VDMA / Agoria / Cetim / Symop avec questions d'acheteurs sans réponse ?

### Takeaway
La FAQ DGUV Test cite textuellement 1.2.1(f) et la durée de 5 ans ; le VDMA publie une FAQ CRA (2e édition) mais pas de FAQ ouverte 1.1.9 ; aucune page Q&A Agoria / Cetim / Symop / TÜV n'a été trouvée avec des questions ouvertes. Les pages n'ont pas pu être lues (hôtes bloqués), donc l'existence de questions « sans réponse » n'est pas vérifiable.

### Cited Findings
- DGUV Test FAQ : cite 1.2.1(f) (5 ans) et, pour l'IA, une conservation d'un an des données du processus décisionnel lié à la sécurité — [DGUV FAQ](https://www.dguv.de/dguv-test/prod-pruef-zert/konform-prod/maschinen/eu-maschinenverordnung/faq-zur-eu-maschinenverordnung/index.jsp)
- VDMA : FAQ CRA 2e édition, « orientation non contraignante » ; guide de gestion des vulnérabilités de l'AK Industrial Security — [VDMA](https://www.vdma.eu/de/cyber-resilience-act)
- IHK Lippe‑Detmold : la machine doit « collecter des preuves lorsqu'on intervient dans son logiciel ou modifie sa configuration » ; protocole et versions accessibles aux autorités jusqu'à 5 ans — [IHK](https://www.ihk.de/lippe-detmold/hauptnavigation/beraten-und-informieren/innovation-und-digitalisierung/aktuelles2/neue-eu-maschinenverordnung-5683128)
- UNM (France) : « Guide cybersécurité des machines 2025 » (PDF) — [UNM](https://unm.fr/wp-content/uploads/2025/04/Guide-cybersecurite-des-machines_2025.pdf) ; CAP'TRONIC webinaire « La cybersécurité dans le nouveau règlement sur les machines » — [automation-hub.fr](https://www.automation-hub.fr/event-details/webinaire-la-cybersecurite-dans-le-nouveau-reglement-sur-les-machines)
- Calendrier réglementaire utile pour l'argumentaire d'urgence (sources secondaires, à recouper) :
  - Application du 2023/1230 : **20 janvier 2027**, sans transition ; les associations industrielles demandent un report des exigences cyber au 11/12/2027, **non adopté** ; une « date de transition 14/01/2029 » parfois citée n'a aucun fondement — [ce-copilot.de](https://www.ce-copilot.de/blog/ot-cyber-security)
  - Règlement (UE) **2026/1744** du 8 juillet 2026 (« Digital Omnibus IA ») modifie 2023/1230 mais **uniquement sur le volet IA** (passage de la section A à la section B de l'annexe I de l'AI Act) ; aucun changement trouvé sur 1.1.9/1.2.1(f) ni sur la date — [EUR-Lex](https://eur-lex.europa.eu/eli/reg/2026/1744/oj/eng) ; [Eurogip](https://eurogip.fr/reglement-machines-2023-1230-ce-que-le-digital-omnibus-sur-lia-change/)
  - EN 50742 : enquête publique close 04/03/2026, E DIN EN 50742 (VDE 0113‑742):2026‑03 avec objections jusqu'au 20/04/2026 ; vote final prévu septembre 2026, **publication annoncée novembre 2026** (Pilz, via Industrie Vandaag) ; pas de présomption de conformité avant citation au JOUE ; IEC 62443 reconnue comme route alternative — [VDE Verlag](https://www.vde-verlag.de/p/normen/e-din-en-50742-vde-0113-742-2026-03/1101006-DE-PR) ; [iTeh](https://standards.iteh.ai/catalog/standards/sist/07a63742-502d-4d13-bdb9-17a86e07b9f8/osist-pren-50742-2026) ; [Industrie Vandaag](https://industrievandaag.nl/en-50742-publicatie-november/)
  - CRA : obligations de notification à partir du **11 septembre 2026**, reste au 11/12/2027 — [TAW](https://www.taw.de/downloads/maschinenverordnung-und-cyber-resilience-act-was-konstrukteure-jetzt-wissen-muessen)
  - Alerte (source presse, non recoupée) : des agences US auraient averti fin août 2026 d'attaques actives sur des S7 dont S7‑1500 — [ad-hoc-news.de](https://www.ad-hoc-news.de/wirtschaft/maschinenverordnung-cybersecurity-pflicht-fuer-hersteller-ab-januar/69851908)

### Inferences
- Le meilleur déclencheur commercial à 30 jours n'est pas un fil de forum mais le **calendrier** : publication EN 50742 en novembre 2026 + 100 jours avant le 20/01/2027. Les fils F1/F2 sont des points d'entrée pour poster, pas des leads chauds.
- Toute réponse postée doit éviter de citer « 5 ans » comme obligation *embarquée* : le texte impose la conservation du journal et des versions, « jusqu'à cinq ans », à disposition des autorités ; la méthode n'est pas prescrite (Apave). OASIS répond au « comment » (journal inaltérable, autorisation par ordre), ce qui est cohérent avec la phase 1.4 / Modbus TCP du CLAUDE.md, mais la couche TCP est « en mémoire seulement » (pas de binaire réseau, pas de journal d'intervention persistant) : ne pas promettre un produit déployable aujourd'hui.

### Gaps
- Pages DGUV, VDMA, Agoria, Cetim, Symop, TÜV non lues (bloquées) : présence de questions ouvertes non vérifiée.
- Aucune page Q&A Agoria / Cetim / Symop trouvée par le moteur pour ces termes.

---

## Synthèse classée (probabilité de pré‑vente sous 30 jours — estimations basses, non vérifiées)

| Rang | Fil | Forum / langue | Date | Besoin | Urgence | Prob. 30 j |
|---|---|---|---|---|---|---|
| 1 | TIA V21 – Secure PG/PC-/HMI-Kommunikation deaktivieren (121228) | sps-forum.de / DE | 2026 (déduit de l'ID, non lu) | Compatibilité panels anciens ; répondants invoquent les règles cyber UE | Aucune échéance citée | ≈ 5 % |
| 2 | NIS2 (!?) End-to-end Kryptierung wird gefordert (112995) | sps-forum.de / DE | ≈ 2023‑24 (ancien) | Client exige chiffrement E2E ; débat sur le périmètre MRL | Exigence client, pas de date | ≈ 5 % (si réactivé) |
| 3 | S7-1500 Modbus-TCP zur Leittechnik (111973) | sps-forum.de / DE | ≈ 2023‑24 (ancien) | Limiter les écritures Modbus d'un superviseur | Aucune | ≈ 3 % |
| 4 | SBOM von SPS-Herstellern (114013) | sps-forum.de / DE | ≈ 2024 (ancien) | SBOM CRA | Aucune | < 3 % |
| 5 | Audit trail / Audit log (CODESYS Forge) | forge.codesys.com / EN | 2017 (ancien) | Audit log intégré CODESYS | Aucune | < 2 % |
| — | Software als Sicherheitsbauteil (118964) | sps-forum.de / DE | ≈ 2025 | Billet Pilz, pas un acheteur | — | 0 % (lieu de publication) |

**Conclusion honnête :** aucun fil trouvé ne révèle « un acheteur avec un besoin urgent et explicite » d'une passerelle d'autorisation de commandes + journal 5 ans. Les forums d'automaticiens FR/DE/NL n'ont pas (encore) absorbé le 1.1.9 ; la demande est au stade formation/webinaire. Actions recommandées : (1) lire F1 et F2 depuis un poste non filtré pour relever auteurs et dates ; (2) poster les réponses proposées ; (3) relancer la recherche après la publication d'EN 50742 (novembre 2026), moment où les questions « comment faire » devraient apparaître ; (4) cibler le workshop Maschinenbautage Köln du 16/10/2026.

---

## Question 5 (extension de périmètre demandée en cours de tâche) — Fils 2026 hors Europe (CH, UK, US, CA, AU, JP, KR, IN, BR, Moyen‑Orient) : authentification des commandes, journaux inaltérables, conformité 2023/1230 / EN 50742 / IEC 62443 pour machines exportées

### Takeaway
L'extension n'a **pas pu être recherchée** : le budget de recherche web partagé était épuisé au moment où la demande est arrivée (deux tentatives refusées) et tout accès direct aux sites est bloqué par le proxy. Seuls les résultats anglophones apparus incidemment dans les requêtes précédentes sont listés ; **aucun d'eux n'est un fil de forum**, et aucun forum non‑européen (plctalk.net, control.com, r/PLC, mrplc.com, forum AutomationDirect, LinkedIn Groups US, forums JP/KR/IN/BR) n'a pu être interrogé.

### Cited Findings (sources anglophones hors forums, trouvées incidemment)
- Instron (US, fabricant d'équipements d'essai), blog septembre 2026 : « EU machinery regulation and cyber resilience act » — exemple d'un exportateur US qui communique sur sa mise en conformité 2023/1230 + CRA — [Instron](https://www.instron.com/en/resources/blog/2026/september/eu-machinery-regulation-and-cyber-resilience-act/)
- Endian (IT/US) : « EU Machinery Regulation Cybersecurity for machines with Endian » — contenu fournisseur (passerelle/firewall), concurrent potentiel sur le positionnement « 1.1.9 » — [Endian](https://www.endian.com/en/resources/communication/blog/eu-machinery-regulation-cybersecurity-for-machines-with-endian/)
- Jama Software (US) : « EU Machinery Regulation: Preparing for Cybersecurity and AI Requirements » — angle gestion des exigences, pas de besoin acheteur — [Jama](https://www.jamasoftware.com/blog/eu-machinery-regulation/)
- sopx.io : « EU Machinery Regulation 2023/1230: OEM Checklist for 2027 » — checklist OEM anglophone ; mentionne que le journal doit inclure le type d'intervention, horodatage/compteurs, preuve de toute suppression, « retained for at least five years and protected against tampering » (lecture de cabinet, à recouper avec le texte) — [sopx.io](https://sopx.io/insights/eu-machinery-regulation-2023-1230/)
- lanpdt.com : « EU Machinery Regulation 2027 – industrial prototype checklist » — [lanpdt](https://lanpdt.com/eu-machinery-regulation-2027-industrial-prototype-checklist/)
- Checkmate Experts (EN) : « Machinery Regulation 2027: Cybersecurity Becomes a CE Requirement » ; conseil de demander aux fournisseurs PLC/variateurs/HMI une certification IEC 62443‑4‑2 — [Checkmate](https://www.checkmate.expert/en/maschinenverordnung-cybersicherheit.html)
- BYHON (Italie, EN) : « EN 50742 vs. IEC 62443 for complying with Machinery Regulation » — [BYHON](https://www.byhon.it/compliance-with-the-machinery-regulation/)
- Sarah Fluchs (admeritia, DE, EN) : « CRA & Machinery regulation » (Medium) — [Medium](https://fluchsfriction.medium.com/cra-machinery-regulation-e4bf14aae5c4)
- regulatorydecoded.com : « prEN 50742: The New Machine Compliance Standard You Need to Know Before 2027 » — [Regulatory Decoded](https://regulatorydecoded.com/how-pren-50742-is-redefining-machine-compliance/)
- Beckhoff USA, communiqués avril et mai 2026 (déjà cités Q1) — [Beckhoff US](https://www.beckhoff.com/en-us/company/press/beckhoff-meets-growing-security-requirements-driven-by-cyber-resilience-act-and-machinery-regulation-2026-05.html)
- arXiv 2026 : « Converging Safety and Security: IO-Link Wireless and OPC UA over 5G under prEN 50742 » (juillet 2026) — signal académique que prEN 50742 est traité hors Europe/industrie — [arXiv](https://arxiv.org/pdf/2607.15840) ; « Cybersecurity Pathways Towards CE-Certified Autonomous Forestry Machines » (notait l'absence de normes harmonisées sous 2023/1230) — [arXiv](https://arxiv.org/pdf/2404.19643) ; « Effects of the CRA on Industrial Equipment Manufacturing Companies » — [arXiv](https://arxiv.org/pdf/2505.14325)
- Référence IEC 62443 pour acheteurs non‑UE : ISA/IEC 62443‑4‑2 cité comme exigence à demander aux fournisseurs de composants (Checkmate, ci‑dessus) ; ISAGCA « 20 pratiques de programmation sécurisée d'automates » (via automation-sense.com, ancien) — [automation-sense.com](https://www.automation-sense.com/blog/automatisme/la-cybersecurite-des-automatismes-industriels.html)

### Inferences
- Les exportateurs non‑UE (ex. Instron, US) communiquent déjà en septembre 2026 sur la conformité 2023/1230 + CRA : la cible « constructeur hors UE qui exporte vers l'UE » existe, mais on ne sait pas si elle pose des questions sur des forums ou traite en interne/avec des cabinets.
- Les contenus anglophones sont presque tous des fournisseurs (Endian, Jama, Beckhoff) ou des cabinets : même constat qu'en Europe, phase « sensibilisation ». Endian et Trout (Q2) sont les deux fournisseurs vus qui occupent explicitement le créneau « passerelle devant l'automate pour 2023/1230 » — à surveiller comme concurrence de positionnement.

### Gaps (recherche à relancer dès que le budget web est disponible)
- Forums US/UK/CA/AU : plctalk.net, control.com, mrplc.com, r/PLC et r/ICS (reddit bloqué pour le robot), forum AutomationDirect, Inductive Automation forum, LinkedIn Groups « Industrial Automation », « ICS Cybersecurity ».
- Suisse : forum Swissmem / electrosuisse ; UK : TheIET forums, Automation Magazine ; Japon : Qiita / Monoist ; Corée : Naver cafés automation ; Inde : plc-forum.in ; Brésil : fórum Clube do Hardware / Mecatrônica Atual ; Moyen‑Orient : LinkedIn EN/AR.
- Requêtes suggérées : « machinery regulation 2027 export EU logging », « 1.1.9 protection against corruption PLC », « tamper-evident log PLC changes », « IEC 62443-4-2 machine builder export Europe », « Modbus write authentication gateway », « signed commands PLC », « EN 50742 question ».
- Aucune citation verbatim d'un acheteur hors UE : rien à ranker pour l'instant.
