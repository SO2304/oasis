# Prompt : relancer T0, T4 et T5 corrigés sur les 3 RP2040

À coller dans Claude Code **sur le PC où les 3 cartes sont branchées**, à la racine
du dépôt, après `git pull` de la branche `claude/eloquent-ptolemy-oojjn0`.

---

Lis `evidence/silicon/2026-10-04/REPORT.md`, en particulier le §13 (errata). Trois
tests du firmware `oasis-silicon-test` ont été corrigés et doivent être relancés
sur les 3 cartes A, B et C :

- **T0** mesure maintenant l'horloge cœur avec SysTick sur une fenêtre de 100 ms
  du TIMER 1 MHz, et ne passe que si elle est à ±1 % de la valeur configurée.
- **T4** ajoute un contrôle négatif : 50 cas nominaux ne doivent jamais être
  bloqués (`nominal_false_blocks=0/50`).
- **T5 v9** modifie un octet couvert par le MAC, l'envoie à un récepteur neuf et
  exige `"bad mesh mac"`. Avant, le rejet venait de l'anti-doublon. T5 v0A exige
  maintenant `"bad mesh signature"`.

Le firmware `uart_mesh.rs` journalise désormais chaque échec CRC.

## Étapes

1. Vérifie que la branche compile : `cargo test --workspace --release` doit passer
   (480 tests au moment de la correction).
2. Pour chaque carte, compile avec son identifiant :
   `OASIS_BOARD_ID=A cargo build --release` dans `oasis-silicon-test`, puis
   `elf2uf2-rs` (voir §8 du rapport). Envoie `b` sur le port série pour passer en
   BOOTSEL. Copie le UF2 **uniquement** sur un volume `RPI-RP2` que tu as identifié.
3. Lance la suite 3 fois par carte (`r`). Enregistre les logs bruts **sans les
   modifier** dans `evidence/silicon/<date du jour>/board_{A,B,C}_run{1,2,3}.log`.
   N'écrase pas les logs du 2026-10-04.
4. Relance une fois le balayage de bruit (`N` sur A, avec `uart_mesh.rs`) et
   archive le log dans le nouveau dossier, pour obtenir le nombre exact d'échecs CRC.
5. Calcule les SHA-256 **après** avoir écrit les fichiers, sur leur contenu final
   (fins de ligne LF), dans `SHA256SUMS`, puis vérifie avec `sha256sum -c`.
6. Écris un `REPORT.md` court dans le nouveau dossier. Pour chaque test et chaque
   carte, indique PASS ou FAIL et les valeurs brutes : `clk_measured`,
   `nominal_false_blocks`, raison du rejet T5. Si un test échoue, écris FAIL avec
   la cause. Ne modifie ni la crypto, ni R14, ni le mesh d'`oasis-rt` pour le
   faire passer : arrête-toi et demande-moi.
7. Si T0, T4 et T5 passent sur les 3 cartes, remplace dans `CLAUDE.md` la mention
   « re-run pending » par un lien vers le nouveau rapport.
8. Commite sur la branche courante, puis pousse.
