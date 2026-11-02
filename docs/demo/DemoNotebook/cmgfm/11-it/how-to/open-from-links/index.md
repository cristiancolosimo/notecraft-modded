---
id: 55ebd3a9aa5bcbd0cd62
title: "Aprire una pagina da un link"
order: 90
created: 2026-10-18T00:00:00Z
updated: 2026-10-18T00:00:00Z
tags: []
---

## Previsto: link che aprono NoteCraft

È previsto che NoteCraft supporti link che aprono una pagina specifica di un quaderno da un
browser, un'email, una chat o un'altra applicazione. Il sistema operativo userà il tipo di link
`notecraft:` per avviare NoteCraft e passargli la pagina da aprire. A seconda del sistema
operativo e del browser, ti potrebbe essere chiesta una conferma prima che l'app si apra.

Questa funzione **non è ancora disponibile**. NoteCraft non registra ancora `notecraft:` nel
sistema operativo e non gestisce un link esterno andando alla sua pagina. Copiare il link di una
pagina di NoteCraft non significa che oggi si possa aprire da un browser.

## Come è prevista la registrazione

- **Programma di installazione per Windows:** registra NoteCraft durante l'installazione e toglie
  la registrazione quando NoteCraft viene disinstallato.
- **ZIP portatile per Windows:** offre un'azione separata **Registra i link di NoteCraft** da
  eseguire a scelta, più un modo per annullarla. Estrarre lo ZIP da solo non registra il link.
- **macOS:** dichiara il tipo di link nel pacchetto dell'app NoteCraft.
- **Linux:** installa un'associazione di applicazione desktop per il tipo di link.

La registrazione dice soltanto al sistema operativo quale applicazione può ricevere un link.
NoteCraft deve anche trovare il quaderno e la pagina corrispondenti, gestire i link quando è già
in esecuzione e spiegare chiaramente quando un quaderno o una pagina non si trovano. Anche questa
gestione fa parte della funzione prevista.
