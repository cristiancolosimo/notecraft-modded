---
title: Grafici, Vega e D3
---

# Grafici, Vega e D3

NoteCraft disegna direttamente sulla pagina i blocchi di grafici Mermaid. Gli esempi qui sotto
sono grafici disegnati. Anche le specifiche Vega e i programmi D3 sono modi utili per descrivere
i grafici, ma per ora NoteCraft li conserva come codice: non li esegue né li disegna come grafici.

## Grafici disegnati

### Barre e una linea

```mermaid
xychart-beta
    title "Pagine scritte ogni mese"
    x-axis [Gen, Feb, Mar, Apr, Mag]
    y-axis "Pagine" 0 --> 100
    bar [20, 38, 55, 72, 90]
    line [12, 29, 48, 68, 86]
```

### Una torta

```mermaid
pie title Come si usano le note
  "Scrittura" : 45
  "Pianificazione" : 30
  "Consultazione" : 25
```

### Un quadrante

```mermaid
quadrantChart
    title Idee da provare
    x-axis Poco sforzo --> Molto sforzo
    y-axis Poco valore --> Molto valore
    quadrant-1 Da fare dopo
    quadrant-2 Da pianificare
    quadrant-3 Da saltare
    quadrant-4 Forse
    Link tra pagine: [0.75, 0.82]
    Strumenti a inchiostro: [0.58, 0.72]
    Temi: [0.25, 0.3]
```

### Un calendario

```mermaid
gantt
  title Un piccolo progetto di quaderno
  dateFormat YYYY-MM-DD
  section Realizzazione
  Bozza :done, a1, 2026-10-01, 2d
  Scrittura :active, a2, after a1, 3d
  section Revisione
  Condivisione :a3, after a2, 1d
```

## Sorgente Vega: un grafico a barre

Questa specifica è mostrata come sorgente JSON, non ancora disegnata da NoteCraft.

```json
{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "data": {
    "values": [
      {"month": "Jan", "pages": 20},
      {"month": "Feb", "pages": 38},
      {"month": "Mar", "pages": 55}
    ]
  },
  "mark": "bar",
  "encoding": {
    "x": {"field": "month", "type": "ordinal"},
    "y": {"field": "pages", "type": "quantitative"}
  }
}
```

## Sorgente Vega: un grafico a linee

```json
{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "data": {
    "values": [
      {"month": "Jan", "pages": 12},
      {"month": "Feb", "pages": 29},
      {"month": "Mar", "pages": 48}
    ]
  },
  "mark": "line",
  "encoding": {
    "x": {"field": "month", "type": "ordinal"},
    "y": {"field": "pages", "type": "quantitative"}
  }
}
```

## Sorgente D3: un grafico a barre

Questo JavaScript è il codice sorgente di un grafico D3. NoteCraft mostra il codice ma non lo esegue.

```javascript
const data = [
  { month: "Jan", pages: 20 },
  { month: "Feb", pages: 38 },
  { month: "Mar", pages: 55 }
];

const svg = d3.select("svg")
  .attr("width", 400)
  .attr("height", 260);
const x = d3.scaleBand()
  .domain(data.map(d => d.month))
  .range([0, 360])
  .padding(0.15);
const y = d3.scaleLinear()
  .domain([0, d3.max(data, d => d.pages)])
  .range([220, 0]);

svg.selectAll("rect")
  .data(data)
  .join("rect")
  .attr("x", d => x(d.month))
  .attr("y", d => y(d.pages))
  .attr("width", x.bandwidth())
  .attr("height", d => 220 - y(d.pages));
```

## Sorgente D3: un grafico a linee

```javascript
const data = [
  { month: "Jan", pages: 12 },
  { month: "Feb", pages: 29 },
  { month: "Mar", pages: 48 }
];

const svg = d3.select("svg")
  .attr("width", 400)
  .attr("height", 260);
const x = d3.scalePoint()
  .domain(data.map(d => d.month))
  .range([20, 360]);
const y = d3.scaleLinear()
  .domain([0, d3.max(data, d => d.pages)])
  .range([220, 0]);
const line = d3.line()
  .x(d => x(d.month))
  .y(d => y(d.pages));

svg.append("path")
  .datum(data)
  .attr("fill", "none")
  .attr("stroke", "currentColor")
  .attr("d", line);
```

Usa i blocchi Mermaid per vedere i grafici nella pagina. Tieni gli esempi Vega e D3 come codice
finché NoteCraft non aggiungerà il supporto a quei formati.

Avanti: [Da destra a sinistra](../right-to-left/).
