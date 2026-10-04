# Kit LinkedIn OASIS : trouver des partenaires pilotes

Tout ce qui suit est prêt à copier-coller dans LinkedIn. Les chiffres viennent de
`CLAUDE.md` et `IOT_READINESS.md`. Ce qui a été fait en simulation est présenté
comme de la simulation. Un partenaire technique vérifiera, et cette honnêteté
renforce la crédibilité du projet.

---

## 1. Titre du profil (Headline, 220 caractères max)

> Fondateur d'OASIS · Autonomie sûre et mesh chiffré pour essaims de drones et IoT (Rust, PX4, LoRa) · Recherche de partenaires pilotes

## 2. Section « Infos » (About)

> Je développe OASIS, une couche logicielle en Rust qui s'installe au-dessus de
> l'autopilote PX4. Elle permet à des drones et à des capteurs IoT d'agir de façon
> autonome et sûre, et de communiquer sans cloud.
>
> Ce que fait OASIS :
> • Bloque toute action physique quand l'incertitude des capteurs dépasse un seuil (règle R14 : 1000 fautes sur 1000 bloquées, décision en 331 ns)
> • Fait circuler les messages sur un réseau maillé chiffré (ChaCha20-Poly1305) et signé (Ed25519), résistant au rejeu et à un nœud compromis
> • Pilote PX4 via MAVLink v2 : armement, décollage et mission à 4 waypoints démontrés en simulation PX4 SITL (altitude tenue à ±6 cm)
> • Compile pour microcontrôleur Cortex-M (RP2040, STM32)
>
> Ce qui le rend vérifiable : 443 tests unitaires, 113 preuves formelles (Kani), et des mesures publiées avec leur dispersion. En A/B contre ROS 2 Jazzy, la messagerie d'OASIS est 28× plus rapide que rclcpp en intra-processus et 279× plus rapide que rclcpp en DDS (16 octets, médianes sur 10 exécutions).
>
> La prochaine étape est le matériel réel : radio LoRa, vol sur Pixhawk, audit externe. Je cherche des partenaires pilotes :
> → intégrateurs et opérateurs de drones sous PX4
> → acteurs IoT/LoRa en agriculture, industrie ou infrastructures
> → laboratoires de robotique et de systèmes embarqués
>
> Écrivez-moi votre cas d'usage en deux lignes.

## 3. Section « Sélection » (Featured)

1. **Vidéo de démonstration** (`partners/demo-mesh-3plat.mp4`, 90 s : mesh fédéré téléphone + PC + 3 drones Webots). Uploadez-la directement sur LinkedIn, qui lit les vidéos natives mieux que les liens.
2. **Lien vers la page partenaires** (`partners/oasis_partenaires.html`, publiée en lien).
3. **PDF « Plan produit IoT »** (`OASIS_IOT_PLAN.pdf`). Il montre la feuille de route et l'honnêteté du projet.
4. **Dépôt GitHub**, si vous décidez de le rendre public.

## 4. Post de lancement (avec la vidéo `demo-mesh-3plat.mp4`)

> Un téléphone, un PC et trois drones partagent ce qu'ils apprennent, sans aucun serveur.
>
> Dans cette vidéo de 90 secondes, chaque appareil fait tourner son propre noyau OASIS. Le téléphone Android utilise ses vrais capteurs. Les drones sont simulés sous Webots. En 5 minutes, 78 résumés du téléphone ont été fusionnés côté drones et le téléphone a reçu 401 messages du réseau. Il n'y a ni cloud, ni broker, ni schéma à négocier.
>
> Le cas d'usage visé : des drones hors vue (BVLOS) qui perdent un capteur en mission et doivent continuer à décider en sécurité.
>
> OASIS est écrit en Rust : 443 tests, 113 preuves formelles, et une messagerie 28× plus rapide que ROS 2 (rclcpp intra-processus, 16 octets).
>
> Prochaine étape : le matériel réel (radio LoRa, vol sur Pixhawk). Je cherche 2 ou 3 partenaires pilotes : opérateurs de drones, acteurs de l'IoT LoRa, laboratoires de robotique. Écrivez-moi.
>
> #drones #Rust #IoT #robotique #edgecomputing #systemesembarques

## 4 bis. Second post, quand vous aurez une vidéo PX4 SITL

> Voici OASIS qui pilote un drone de bout en bout, en simulation.
>
> Dans cette vidéo, OASIS arme PX4, déclenche le décollage, enchaîne une mission à 4 waypoints et tient l'altitude à 1,94 m pour une consigne de 2,00 m. Tout passe par MAVLink v2.
>
> OASIS est une couche d'autonomie écrite en Rust qui s'installe au-dessus de l'autopilote. Son principe central : si l'incertitude des capteurs devient trop forte, aucune action physique n'est exécutée.
>
> Ce qui est vérifié aujourd'hui :
> ✅ 443 tests et 113 preuves formelles
> ✅ Réseau maillé chiffré et signé, résistant à un nœud compromis
> ✅ 28× plus rapide que ROS 2 (rclcpp intra-processus) sur la messagerie à 16 octets
>
> Ce qui ne l'est pas encore : un vol sur un vrai Pixhawk et une radio LoRa sur matériel. C'est la prochaine étape.
>
> Je cherche 2 ou 3 partenaires pilotes : opérateurs de drones, acteurs de l'IoT LoRa, laboratoires de robotique. Si vous avez un terrain d'essai et un cas d'usage, écrivez-moi.
>
> #drones #PX4 #Rust #IoT #LoRa #robotique #systemesembarques #cybersecurite

## 5. Messages d'approche (300 caractères max pour une invitation)

**Intégrateur ou opérateur de drones**
> Bonjour [Prénom], je développe OASIS, une couche d'autonomie Rust au-dessus de PX4 (mesh chiffré, blocage des actions en cas d'incertitude). Démontré en SITL, je cherche un partenaire pour un premier vol réel. Cela vous intéresserait-il d'en parler 15 min ?

**IoT / LoRa**
> Bonjour [Prénom], je travaille sur OASIS, un mesh LoRa signé et sans cloud pour capteurs Cortex-M (anti-rejeu, duty-cycle EU868). Je monte un pilote sur 3 nœuds réels. Vos déploiements terrain pourraient-ils être un bon cas d'usage ?

**Laboratoire / R&D**
> Bonjour [Prénom], OASIS est un runtime Rust pour la robotique, avec 113 preuves formelles et une comparaison A/B avec ROS 2. Je cherche un labo partenaire pour la validation sur matériel. Seriez-vous ouvert à un échange ?

## 6. Qui cibler (recherche LinkedIn)

- Titres : « drone operations », « UAV engineer », « PX4 », « responsable innovation », « IoT LoRa », « ingénieur systèmes embarqués », « directeur R&D robotique »
- Écosystème : utilisateurs et contributeurs PX4/Dronecode, pôles de compétitivité (aéronautique, agritech), incubateurs deeptech, laboratoires universitaires de robotique
- Rythme conseillé : 10 à 15 invitations personnalisées par jour, un post par semaine (vidéo, puis un chiffre, puis la feuille de route)

## 7. Avant de publier : à vérifier

- [ ] Dans la vidéo, la courbe finale « PX4 baseline vs OASIS » est synthétique. Si on vous interroge, dites-le, ou retirez-la dans une v3.
- [ ] La vidéo affiche des chiffres plus anciens (« 130 unit tests », « ~5 500 LoC », « 28h+ phone session »). Une v3 avec les chiffres actuels (443 tests) serait plus solide.
- [ ] Ne pas écrire « testé sur matériel réel » pour le drone. Le run Android de 3h23 n'a pas de journaux archivés, donc ne pas le mettre en avant.
- [ ] Décider si le dépôt GitHub devient public avant de le lier
