# OASIS face aux concurrents, axe par axe (2026-10-06)

Méthode : les affirmations sur OASIS viennent du code (`oasis-rt/src/mesh.rs` sur
`main`) et des logs silicium. Les affirmations sur les concurrents viennent de leur
code ou de leur documentation, sources en bas. « À vérifier » signifie que je n'ai
pas trouvé de source directe.

Légende : ✅ fait · ⚠️ partiel ou avec une faiblesse · ❌ absent · — sans objet

## 1. Les axes où OASIS veut gagner

| Axe | OASIS aujourd'hui (v0A) | OASIS après v0B + révocation | Bluetooth Mesh | Thread | Meshtastic | Reticulum | LoRaWAN |
|---|---|---|---|---|---|---|---|
| **A. Chaque relais authentifie l'origine avec une clé propre au nœud** (un nœud capturé ne peut pas se faire passer pour un autre) | ✅ en-tête seulement, prouvé sur silicium | ✅ | ❌ Les relais vérifient le NetMIC avec la clé réseau **partagée** : tout détenteur de la clé peut forger | ❌ Clé réseau partagée | ❌ Les relais ne vérifient pas ; clé de canal partagée ; messages directs vérifiés à l'arrivée seulement | ❌ Aucune vérification cryptographique des données par les relais (`Transport.py` l.2018-2045) | — Pas de relais ; les passerelles retransmettent sans vérifier, le serveur vérifie |
| **B. Chaque relais vérifie l'intégrité du contenu** | ❌ Le contenu n'est pas signé (12/50 inversions acceptées sur silicium) | ✅ Empreinte du contenu signée | ⚠️ Le NetMIC couvre le paquet, mais avec la clé partagée | ⚠️ Idem, clé partagée | ❌ | ❌ | — |
| **C. Anti-rejeu qui survit à un redémarrage** | ❌ Cache de doublons en RAM uniquement : un vieux message signé est réaccepté après redémarrage ou remise à zéro du Bloom | ✅ Compteur signé + fenêtre par origine, persistés | ✅ Numéro de séquence et liste anti-rejeu persistés. ⚠️ Mais un détenteur de la clé réseau peut **saturer la liste anti-rejeu** avec de fausses entrées et bloquer les messages légitimes | ✅ Compteurs de trame (persistance : à vérifier) | ⚠️ Identifiant de paquet de 32 bits et anti-doublon ; persistance non documentée | ⚠️ Liste d'empreintes de paquets (persistance : à vérifier) | ✅ Compteurs de trame persistés |
| **D. Un nœud compromis ne peut pas bloquer les messages des autres** | ❌ Remplacement du contenu → le vrai message est écarté comme doublon | ✅ Un message n'entre dans l'état du relais qu'après vérification complète | ❌ Saturation de la liste anti-rejeu possible avec la clé réseau | ❌ Clé partagée | ❌ | ⚠️ Rien n'est vérifié en transit ; seule la destination filtre | — |
| **E. Révoquer un seul nœud sans recléfier tout le réseau** | ❌ Révocation signée (v6) côté `spore`, **`std` seulement**, et **non appliquée par les relais mesh** | ✅ Si on l'implémente en `no_std` et appliquée au relais | ⚠️ « Key Refresh » : on renouvelle la clé de tous les nœuds sauf l'exclu. Lourd mais standard | ⚠️ Changement de clé réseau pour tous | ❌ Changement manuel de la clé de canal | ⚠️ Listes « blackhole » locales ou par abonnement, sans révocation globale (choix assumé) | ✅ Désactivation côté serveur (centralisé) |
| **F. Sûreté des actionneurs liée aux communications** (aucune action si capteurs peu fiables, MAVLink/PX4) | ✅ Porte R14, pont MAVLink | ✅ + ordre signé obligatoire (voir §3) | ❌ | ❌ | ❌ | ❌ | ❌ |
| **G. Langage sûr en mémoire + preuves formelles** | ✅ Rust, 113 harnais Kani | ✅ | ⚠️ Implémentations en C (ex. Zephyr), fuzzées et qualifiées SIG | ⚠️ OpenThread en C++, fuzzé et certifié | ❌ C++ | ⚠️ Python ; microReticulum en C++ | ⚠️ C, certifié |
| **H. Preuves brutes publiées sur silicium** | ✅ Logs, empreintes SHA-256, firmware | ✅ | Certification à la place | Certification | ❌ | ❌ | Certification |

## 2. Les axes où OASIS perd, et c'est assumé

| Axe | Qui gagne | Pourquoi on ne se bat pas ici |
|---|---|---|
| Débit et coût par saut | Tous. Ils utilisent AES-CCM ou HMAC (quelques µs) ; OASIS fait une vérification Ed25519 (176 ms par saut sur Cortex-M0+) | C'est le prix de A. On choisit la sûreté. |
| Anonymat de l'émetteur | Reticulum | OASIS doit savoir qui parle pour filtrer au relais : c'est incompatible par construction. |
| Certification, écosystème, radio réelle, énergie | Bluetooth Mesh, Thread, LoRaWAN, Meshtastic | Des années d'avance. OASIS a 0 utilisateur et pas de radio testée. |
| Protection de la clé sur le nœud | Égalité basse : la plupart tournent sur des puces sans protection activée par défaut | À rattraper avec le RP2350 (OTP, secure boot), pas à revendiquer. |

## 3. Comment devenir « injouable » sur A à F

Aucun concurrent ne coche A, B, C, D et E en même temps : soit la clé est partagée
(Bluetooth Mesh, Thread), soit rien n'est vérifié en transit (Reticulum,
Meshtastic). C'est la place à prendre. Dans l'ordre :

1. **v0B** (prompt `prompts/MESH_V0B_SECURITY.md`) : contenu, compteur et
   identifiant de réseau signés ; état modifié seulement après vérification ;
   persistance. Ferme B, C et D.
2. **Révocation au niveau mesh, en `no_std`** : liste signée par la clé
   opérateur, propagée dans le mesh, appliquée par chaque relais avant la
   vérification de signature, persistée. Ferme E sans recléfier le réseau, ce
   qu'aucun concurrent en mesh ne fait.
3. **Lier R14 aux ordres authentifiés** : un actionneur ne bouge que si l'ordre
   est signé, frais, émis par une origine autorisée et non révoquée, **et** si
   R14 l'autorise. Avec une preuve Kani de cette règle. Ça rend F unique et
   difficile à copier, parce que les concurrents sont des réseaux, pas des
   systèmes d'actionnement.
4. **Une suite de tests d'attaque publique et rejouable** (rejeu après
   redémarrage, remplacement de contenu, saturation de l'anti-rejeu, nœud
   révoqué) exécutée contre OASIS **et** contre Bluetooth Mesh (Zephyr) et
   Reticulum, avec les logs. C'est ce qui transforme « on est meilleurs » en
   fait vérifiable.
5. **RP2350** : clé en OTP et secure boot, pour que « un nœud capturé ne
   compromet que lui-même » ne repose pas uniquement sur la signature par nœud.

## Sources

- Reticulum : code source `markqvist/Reticulum`, commit `e40191b` (`RNS/Transport.py`, `docs/source/understanding.rst`) ; [manuel Reticulum (PDF)](https://reticulum.network/manual/Reticulum%20Manual.pdf) ; [Using Reticulum (blackhole)](https://reticulum.network/manual/using.html)
- Bluetooth Mesh : [Mesh Protocol 1.1](https://www.bluetooth.com/wp-content/uploads/Files/Specification/HTML/MshPRT_v1.1/out/en/index-en.html) ; [Mesh Security Overview (SIG)](https://www.bluetooth.com/wp-content/uploads/2025/04/MeshSecurityOverview_INFO_v1.0-1.pdf) ; [Zephyr Bluetooth Mesh Core](https://docs.zephyrproject.org/latest/services/connectivity/bluetooth/api/mesh/core.html) ; [Silicon Labs, Sequence Number and IV Index](https://docs.silabs.com/btmesh/latest/btmesh-iv-update/02-sequence-number-and-iv-index)
- Thread : [Silicon Labs, OpenThread Security](https://docs.silabs.com/openthread/latest/thread-fundamentals/09-security)
- Meshtastic : [Updated Security Implementation](https://meshtastic.org/docs/development/reference/encryption-technical/) ; [Encryption limitations](https://meshtastic.org/docs/about/overview/encryption/limitations/) ; [Mesh Broadcast Algorithm](https://meshtastic.org/docs/overview/mesh-algo/)
- microReticulum : [Reticulum Community Wiki](https://reticulum.miraheze.org/wiki/MicroReticulum)
