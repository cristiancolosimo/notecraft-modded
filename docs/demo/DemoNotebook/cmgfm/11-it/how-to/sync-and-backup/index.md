---
id: 24de4ec846650a8fa8cf
title: "Sincronizzazione e backup"
order: 60
created: 2026-10-18T00:00:00Z
updated: 2026-10-18T00:00:00Z
tags: []
---

NoteCraft non ha una sincronizzazione propria e non ha account. Un quaderno è una cartella di
piccoli file, quindi metti la cartella dei quaderni dove il tuo strumento di sincronizzazione
già guarda: OneDrive, Dropbox, Google Drive, iCloud Drive, Syncthing, una condivisione di rete
o un repository git. Nessuno di loro deve capire NoteCraft. Copiano file.

## Perché funziona

- **Un file per pagina.** Modificare una pagina riscrive un piccolo file, quindi due computer
  raramente toccano gli stessi byte.
- **Scritture atomiche.** Una pagina viene scritta in un file temporaneo e rinominata sopra
  quello vecchio, così un client di sincronizzazione non vede mai una pagina a metà.
- **Nomi brevi e sicuri.** I nomi delle cartelle hanno al massimo 64 caratteri, i percorsi al
  massimo 255, e non viene mai scritto nulla che Windows rifiuti.
- **Le copie in conflitto vengono notate.** Quando uno strumento di sincronizzazione lascia una
  copia in conflitto accanto a una pagina, il controllo la segnala: **File**, poi **Controlla**,
  oppure `notecraft-cli check` da terminale.

## Backup

Un backup è una copia della cartella. Non c'è altro da esportare.

## Cronologia della pagina

Apri **Cronologia**, poi **Versioni della pagina**, per rivedere e ripristinare le copie salvate
di questa pagina. Per recuperare una pagina, sezione o gruppo di sezioni eliminati, apri
**Cronologia**, poi **Cestino del quaderno**. Questi strumenti funzionano su questo dispositivo e
non sostituiscono un backup separato del quaderno.

## Cosa non è gestito

NoteCraft non unisce due versioni di una pagina. Se il tuo strumento di sincronizzazione crea una
copia in conflitto, decidi tu quale versione tenere.

Avanti: [Personalizza](../make-it-yours/).
