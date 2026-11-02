---
id: 88ad8a4fdfe68e0c6954
title: "Immagini e file"
order: 40
created: 2026-10-18T00:00:00Z
updated: 2026-10-18T00:00:00Z
tags: []
---

Trascina un file dal tuo gestore di file su questa pagina. NoteCraft chiede cosa farne:

- **Inserisci l'immagine**: una copia va nella cartella `assets` di questa pagina e la pagina la mostra.
- **Allega il file**: la stessa copia, mostrata come link.
- **Collega il file dov'è**: non si copia nulla, e il link si rompe se il file viene spostato.

**Inserisci**, poi **Immagine** o **File** fa lo stesso e ti dice di trascinare il file.

## Dove finisce

```text
pictures-and-files/
  index.md
  assets/
    diagram.png
    budget.xlsx
```

L'immagine viaggia con la pagina: sposti la cartella e l'immagine si sposta con lei. I nomi
vengono resi sicuri per ogni sistema operativo, due file non hanno mai lo stesso nome, e un file
oltre 512 MB viene rifiutato.

## Incollare

Una tabella copiata da un'altra app di note o da un foglio di calcolo si incolla come tabella.
Per incollare un'immagine, clicca nella pagina e premi **Ctrl+V**. NoteCraft salva una copia
nella cartella `assets` della pagina e la inserisce nella nota.

## Stampe

Alcune app sanno mostrare un PDF o un documento come immagini sulla pagina. NoteCraft non
può ancora farlo; un PDF che alleghi resta un file, e la finestra lo dice.

Avanti: [Due formati di pagina](../page-formats/).

![Ink](ink.svg)
