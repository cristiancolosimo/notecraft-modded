---
title: Diagrammi Mermaid
---

# Diagrammi Mermaid

Scrivi un diagramma come testo Mermaid in un blocco delimitato. NoteCraft disegna il blocco sulla
pagina nel tema corrente; apri il sorgente della pagina con **< >** per modificare il testo.

## Diagramma di flusso

```mermaid
flowchart TD
  Start[Scrivi una nota] --> Save{Salvata?}
  Save -->|sì| Ready[Continua]
  Save -->|non ancora| Wait[Fai una breve pausa]
  Wait --> Save
```

## Sequenza

```mermaid
sequenceDiagram
  Autore->>Pagina: Scrive una riga
  Pagina->>File: Salva la pagina
  File-->>Pagina: Salvata
  Pagina-->>Autore: Continua a scrivere
```

## Mappa mentale

```mermaid
mindmap
  root((Un quaderno))
    Pagine
      Parole
      Equazioni
    Disegni
      Forme
      Inchiostro
    File
      Immagini
      Allegati
```

## Diagramma di stato

```mermaid
stateDiagram-v2
  [*] --> Pronto
  Pronto --> Modifica: scrive
  Modifica --> Salvato: pausa
  Salvato --> Modifica: scrive ancora
  Salvato --> [*]
```

## Diagramma a torta

```mermaid
pie title Pagine di questo percorso
  "Scrittura" : 4
  "Disegno" : 2
  "File" : 2
```

Ogni blocco è un diagramma a sé. Il testo resta nel file della pagina, quindi si può cambiare o
copiare senza un editor di immagini.

Avanti: [Grafici, Vega e D3](../charts-vega-d3/).
