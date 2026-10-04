# SHADOW AUDIT — Prompt « binaire de test + mode d'emploi illustré PDF »

**Date :** 2026-06-12 · **Posture :** honnêteté brutale, Avocat du Diable sur la demande elle-même.
**Objet :** auditer le *prompt* (pas seulement y répondre) et exposer les limites de mes propres livrables.

---

## La demande (reformulée)
« Binaire pour le test + mode d'emploi très détaillé illustré (une illustration par étape) en PDF
+ shadow audit du prompt. »

## Verdict en une ligne
**Intention saine, mais une hypothèse fausse implicite :** « un binaire peut tester l'IoT
*maintenant* ». Non. Le vrai test IoT est matériel, et le driver radio n'existe pas encore. J'ai livré
le maximum *honnête* — un POC **logiciel** réel + un protocole matériel **schématique** — en étiquetant
clairement ce qui est prouvé et ce qui ne l'est pas.

---

## Constats (impitoyables)

### 1. « Binaire pour le test » — la vraie réponse est inconfortable
Le test IoT qui compte se fait sur **silicium + radio réels**. Or le **driver SX1262 est un STUB**
(`sx1262.rs::init()` → « not implemented »). **Conséquence : aucun binaire ne teste la radio réelle
aujourd'hui.** Ce que j'ai produit (`mesh_poc.exe`) est un **POC logiciel sur PC, radio simulée** : il
prouve la *logique* (signer v0A → tramer → « émettre » → recevoir → **vérifier**, rejet d'une forge,
airtime réel, budget duty-cycle) — **pas** la radio. Si tu attendais un firmware flashable faisant du
vrai LoRa, **il n'existe pas** et le prétendre serait du théâtre.

### 2. « Illustration pour chaque étape » — je n'ai ni banc ni caméra
Mes illustrations sont des **schémas SVG dessinés à la main** (topologie mesh, câblage Pico+SX1262, flux
cryptographique, profil de courant attendu, maquette de sortie console). Elles sont **précises et utiles**,
mais **schématiques** — ce ne sont **pas** des photos d'un vrai banc, ni des captures réelles
d'oscilloscope/SDR/PPK2. Si tu voulais des photos du matériel réel, **c'est hors de ma portée** (je
n'exécute aucun matériel).

### 3. Le mode d'emploi décrit des étapes que je n'ai NI exécutées NI vérifiées
Numéros de broches, commandes de flash, mesures de courant : écrits depuis les **datasheets et conventions**,
pas depuis un banc réel. **À vérifier contre tes cartes réelles avant de suivre aveuglément.** Risque
concret : une erreur de pinout ou de commande peut te faire perdre du temps, voire (au pire) stresser une
carte. Le document est un **protocole proposé**, pas une procédure éprouvée.

### 4. « Format PDF » — je ne peux pas le VOIR
Je valide la **structure** du PDF (en-tête `%PDF`, fin `%%EOF`, taille), pas le **rendu visuel**. À ouvrir
pour confirmer que schémas et tableaux s'affichent comme prévu.

### 5. Périmètre : ça valide UNE tranche, et la moitié est conditionnelle
Tout ceci concerne **le nœud LoRa-mesh** uniquement (pas la face drone/PX4, pas le daemon téléphone). La
**Phase A (logiciel)** est lançable *maintenant*. La **Phase B (matériel)** est **conditionnée à l'écriture
du driver SX1262** (non faite) **et** à l'achat du matériel. Le prompt sous-entend un test prêt-à-lancer ;
seule la moitié l'est.

### 6. « Shadow audit le prompt » — excellent réflexe (un seul bémol)
Demander une auto-critique du prompt est exactement la bonne posture : ça m'empêche de sur-livrer du
théâtre pour « faire plaisir ». **Bémol :** un shadow audit n'a de valeur que si tu **agis** dessus.
S'il ne change rien à la suite, c'est de l'honnêteté décorative.

### 7. Risque de second ordre — le prototype éternel
Accumuler des docs et des binaires « POC » sans jamais acheter le matériel = exactement le **risque #3 de
l'autopsie anticipée** (prototype qui ne sort jamais). **Le POC logiciel vert ne doit pas devenir un
substitut au test réel.** C'est un *pré-vol*, pas un vol.

---

## Limites assumées de mes livrables (rien caché)

| Livrable | Ce que c'est | Ce que ce n'est PAS |
|---|---|---|
| `mesh_poc.exe` | POC logiciel : crypto+mesh+airtime+duty-cycle, radio simulée | un test radio réel (driver = stub) |
| `duty_cycle.exe` | démo airtime/duty-cycle (calcul exact) | une mesure RF réelle |
| Mode d'emploi PDF | protocole illustré (SVG), Phase A exécutable + Phase B proposée | une procédure matérielle éprouvée par moi |
| Illustrations | schémas SVG dessinés | photos / captures réelles |

## Ce qu'il faut retenir
1. **Phase A est réelle et lançable aujourd'hui** — fais-la, elle prouve la logique.
2. **Phase B exige du code (driver) + du matériel** — ni l'un ni l'autre n'existe encore ; ne confonds pas
   « POC logiciel OK » avec « validé IoT ».
3. **Vérifie le pinout/les commandes** du mode d'emploi contre tes cartes avant de brancher.
4. **Agis sur ce shadow audit** : la prochaine étape à valeur réelle est *écrire le driver SX1262 + acheter
   3 Pico*, pas produire un document de plus.
