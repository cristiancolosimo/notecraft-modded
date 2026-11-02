---
id: f7eebdc4b57e23a9b601
title: "Guardare dentro una pagina"
order: 75
created: 2026-10-18T00:00:00Z
updated: 2026-10-18T00:00:00Z
tags: []
---

Molte app di note tengono un intero quaderno in un solo file binario, e un editor di testo mostra
solo rumore. Una pagina di NoteCraft è una cartella di file semplici, e la vista codice ti mostra
quella cartella, file per file. Provalo ora: premi il pulsante `< >` accanto a questa pagina.

## Cosa vedrai

A sinistra, **File di questa pagina**: prima il file della pagina, `index.md`, poi tutto il resto
della cartella. Un file che nulla nella pagina usa è segnato *non usato dalla pagina*. A destra
c'è il file che hai scelto. Questa è la pagina che stai leggendo, quindi il visualizzatore mostra
il suo stesso testo.

Il file della pagina è mostrato per intero: la sua **intestazione** (id, titolo, ordine, date e
impostazioni della pagina) e sotto il **corpo**. Puoi modificare tutto. Premi di nuovo lo stesso
pulsante per chiudere la vista, e la pagina viene ridisegnata da ciò che hai scritto.

| File | Cosa mostra il visualizzatore | Puoi scriverci? |
|---|---|---|
| Il file della pagina, `index.md` o `index.html` | L'intestazione e il corpo esattamente come salvati | Tutto |
| Un disegno, `ink.svg` | Il testo SVG, con le letture della penna | Sì, con cautela |
| Un'immagine | L'immagine | No |
| Altro testo | Il suo testo | No |
| Qualsiasi altra cosa | Nome, dimensione e tipo | No |

Un file oltre 256 KB mostra la prima parte e lo dice.

## Cosa ti mostra

- Una pagina organizzata in riquadri ha un `div` con `class="nc-box"` attorno a ogni riquadro.
- Un disegno è un file, `ink.svg`, e il testo della pagina lo nomina con una riga.
- Etichette e link sono testo semplice.

Nulla è nascosto: se la pagina lo mostra, un file di questa cartella lo contiene.

Avanti: [Da destra a sinistra](../right-to-left/).
