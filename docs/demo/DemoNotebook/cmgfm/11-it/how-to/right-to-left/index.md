---
id: 6415225f7e0c52b1c5e5
title: "Da destra a sinistra"
order: 80
created: 2026-10-18T00:00:00Z
updated: 2026-10-18T00:00:00Z
tags: []
---

Arabo, ebraico, persiano e urdu si leggono da destra. NoteCraft li disegna così: le lettere si
uniscono, una riga pende dal bordo destro, e una riga che mescola le due direzioni è composta
dall'algoritmo Unicode, così un prezzo mantiene il suo numero nel verso giusto:

السعر 123 ريال

I caratteri che lo rendono possibile sono inclusi nell'app, quindi la riga qui sopra ha lo stesso
aspetto su un computer senza caratteri arabi installati. Anche ebraico e hindi hanno i propri
caratteri inclusi:

שלום עולם

नमस्ते दुनिया

## Tre direzioni, tre controlli

| Controllo | Dove | Cosa imposta |
|---|---|---|
| **Pagina** | **Home**, poi **Direzione** | In che verso si legge tutta la pagina. Una riga senza lettere proprie la segue |
| **Paragrafo** | Il menu di allineamento in **Testo di base** | In che verso si legge il paragrafo in cui si trova il cursore |
| **Tratto** | **Altro...** nel gruppo Direzione | In che verso si legge la selezione, o ciò che scrivi dopo |

La direzione della pagina è propria di NoteCraft; le app da cui si arriva non hanno una simile
impostazione, e un quaderno scritto in arabo ne ha bisogno.

## Dove viene conservata l'impostazione

Nel testo stesso. La direzione della pagina è `dir: rtl` nel front matter di una pagina Markdown
o un tag meta in una pagina HTML. La direzione di un paragrafo è un carattere invisibile al suo
inizio, il segno Unicode da destra a sinistra o da sinistra a destra. Entrambe sopravvivono a
qualsiasi altro editor.

Una pagina scritta in arabo, persiano, urdu o ebraico è composta da destra a sinistra per intero.
Torna alla [pagina iniziale](../../../00-start/welcome/).
