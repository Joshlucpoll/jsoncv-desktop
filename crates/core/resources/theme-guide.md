# Writing a jsoncv theme

A theme turns a CV (JSON) into one HTML page. The same page is the live preview and the source of the PDF.

## Files

A theme is a folder in `<library>/themes/<id>/`. The id uses lowercase letters, digits and `-`.

```
themes/my-theme/
  theme.json
  template.html.j2
  style.css
  assets/          (optional: fonts, images)
```

`theme.json`:

```json
{ "name": "My theme", "description": "One line about the look.", "page": { "size": "A4" } }
```

`page.size` is `A4` or `Letter`. Built-in themes (ids that start with `builtin:`) are read-only. Copy one into your library to change it.

## Template

`template.html.j2` is a [MiniJinja](https://docs.rs/minijinja) (Jinja2) template. It receives one variable, `cv`: the whole CV document, as the jsoncv schema defines it.

- `cv.basics.name`, `cv.basics.profiles[0].url`
- `cv.work`, `cv.education`, `cv.skills`, `cv.projects` and the other sections are arrays.
- A missing section is undefined, so `{% if cv.work %}` is false for both a missing and an empty section. Use it to hide section headings.
- Output is HTML-escaped by default. Text such as `<b>` shows as text.

Write a full HTML document with a `<head>`. The renderer adds your CSS as a `<style>` element at the end of `<head>`.

## Helpers

| Helper | Use | Example | Result |
| --- | --- | --- | --- |
| `format_date(fmt)` | Format an ISO date (`YYYY`, `YYYY-MM`, `YYYY-MM-DD`). Tokens: `%Y %m %d %b %B`. | `{{ "2021-03" \| format_date("%b %Y") }}` | `Mar 2021` |
| `markdown` | Render Markdown to safe HTML. Raw HTML in the text is shown as text. Links keep only `http`, `https`, `mailto`, `tel` and relative targets. | `{{ job.summary \| markdown }}` | `<p>…</p>` |
| `safe_url` | Make a URL safe for `href` or `src`. Keeps `http`, `https`, `mailto`, `tel` and relative targets. Any other scheme (such as `javascript:`) becomes `#`. Always use it on CV-supplied links. | `<a href="{{ cv.basics.url \| safe_url }}">` | `<a href="https://example.com">` |
| `strip_scheme` | Remove `https://` or `http://` and a trailing `/`. | `{{ cv.basics.url \| strip_scheme }}` | `example.com` |

A date that does not match one of the three formats prints unchanged. A missing date prints nothing. Show "Present" for a missing end date:

```jinja
{{ w.startDate | format_date("%b %Y") }} – {{ w.endDate | format_date("%b %Y") if w.endDate else "Present" }}
```

## Assets

Put fonts and images in `assets/` and refer to them by relative path, for example `url("assets/Inter.woff2")` in `style.css` or `<img src="assets/logo.png">` in the template. The renderer inlines each file as a `data:` URI, so the HTML is self-contained. Paths that contain `..` or that are absolute are rejected. The same inlining applies to local image paths in `cv.basics.image`.

## Page rules

Set the paper in your CSS and keep it in step with `theme.json`:

```css
@page { size: A4; margin: 14mm 14mm; }
```

- Do not use fixed-height page wrappers. Let content flow across pages.
- Add `break-inside: avoid` to entries so a job does not split across pages.
- Add `-webkit-print-color-adjust: exact; print-color-adjust: exact;` if you use background colors.
- Use no external URLs. Use system font stacks or bundled `assets/`.

## Example: the `classic` theme

This is the complete built-in theme. Copy it as a start.

### template.html.j2

```jinja
{% macro dates(s, e) %}{% if s or e %}{% if s %}{{ s | format_date("%b %Y") }} – {% endif %}{{ e | format_date("%b %Y") if e else "Present" }}{% endif %}{% endmacro %}
{% macro link(u) %}<a href="{{ u | safe_url }}">{{ u | strip_scheme }}</a>{% endmacro %}
{% macro tags(list) %}{% if list %}<p class="tags">{% for k in list %}<span>{{ k }}</span>{% endfor %}</p>{% endif %}{% endmacro %}
{% set b = cv.basics or {} %}
{% set loc = b.location or {} %}
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{{ b.name }}{% if b.label %} – {{ b.label }}{% endif %}</title>
</head>
<body>
<main class="cv">
  <header class="masthead">
    <div>
      <h1>{{ b.name }}</h1>
      {% if b.label %}<p class="label">{{ b.label }}</p>{% endif %}
      <ul class="contact">
        {% if b.email %}<li><a href="mailto:{{ b.email }}">{{ b.email }}</a></li>{% endif %}
        {% if b.phone %}<li>{{ b.phone }}</li>{% endif %}
        {% set place = [loc.address, loc.city, loc.region, loc.postalCode, loc.countryCode] | select | join(", ") %}
        {% if place %}<li>{{ place }}</li>{% endif %}
        {% if b.url %}<li>{{ link(b.url) }}</li>{% endif %}
        {% for p in b.profiles %}<li><a href="{{ p.url | safe_url }}">{{ p.network }}{% if p.username %}: {{ p.username }}{% endif %}</a></li>{% endfor %}
      </ul>
    </div>
    {% if b.image %}<img class="photo" src="{{ b.image }}" alt="">{% endif %}
  </header>

  {% if b.summary %}<div class="summary md">{{ b.summary | markdown }}</div>{% endif %}

  {% if cv.work %}
  <section>
    <h2>Work</h2>
    {% for w in cv.work %}
    <article class="entry">
      <div class="row"><h3>{{ w.position or w.name }}</h3><span class="dates">{{ dates(w.startDate, w.endDate) }}</span></div>
      <p class="sub">{% if w.position %}<span>{{ w.name }}</span>{% endif %}{% if w.location %}<span>{{ w.location }}</span>{% endif %}{% if w.url %}<span>{{ link(w.url) }}</span>{% endif %}</p>
      {% if w.description %}<div class="md">{{ w.description | markdown }}</div>{% endif %}
      {% if w.summary %}<div class="md">{{ w.summary | markdown }}</div>{% endif %}
      {% if w.highlights %}<ul class="bullets">{% for h in w.highlights %}<li>{{ h | markdown }}</li>{% endfor %}</ul>{% endif %}
    </article>
    {% endfor %}
  </section>
  {% endif %}

  {% if cv.projects %}
  <section>
    <h2>Projects</h2>
    {% for p in cv.projects %}
    <article class="entry">
      <div class="row"><h3>{{ p.name }}</h3><span class="dates">{{ dates(p.startDate, p.endDate) if p.startDate or p.endDate }}</span></div>
      <p class="sub">{% if p.type %}<span>{{ p.type }}</span>{% endif %}{% if p.entity %}<span>{{ p.entity }}</span>{% endif %}{% if p.roles %}<span>{{ p.roles | join(", ") }}</span>{% endif %}{% if p.url %}<span>{{ link(p.url) }}</span>{% endif %}</p>
      {% if p.description %}<div class="md">{{ p.description | markdown }}</div>{% endif %}
      {% if p.highlights %}<ul class="bullets">{% for h in p.highlights %}<li>{{ h | markdown }}</li>{% endfor %}</ul>{% endif %}
      {{ tags(p.keywords) }}
    </article>
    {% endfor %}
  </section>
  {% endif %}

  {% if cv.education %}
  <section>
    <h2>Education</h2>
    {% for e in cv.education %}
    <article class="entry">
      <div class="row"><h3>{{ e.institution }}</h3><span class="dates">{{ dates(e.startDate, e.endDate) }}</span></div>
      <p class="sub">{% set deg = [e.studyType, e.area] | select | join(", ") %}{% if deg %}<span>{{ deg }}</span>{% endif %}{% if e.score %}<span>{{ e.score }}</span>{% endif %}{% if e.url %}<span>{{ link(e.url) }}</span>{% endif %}</p>
      {% if e.courses %}<p class="muted">{{ (e.courses or []) | join(" · ") }}</p>{% endif %}
    </article>
    {% endfor %}
  </section>
  {% endif %}

  {% if cv.skills %}
  <section>
    <h2>Skills</h2>
    <dl class="pairs">
      {% for s in cv.skills %}
      <div class="entry"><dt>{{ s.name }}{% if s.level %} <small>({{ s.level }})</small>{% endif %}</dt><dd>{{ (s.keywords or []) | join(" · ") }}{% if s.summary %}<span class="md">{{ s.summary | markdown }}</span>{% endif %}</dd></div>
      {% endfor %}
    </dl>
  </section>
  {% endif %}

  {% if cv.volunteer %}
  <section>
    <h2>Volunteer</h2>
    {% for v in cv.volunteer %}
    <article class="entry">
      <div class="row"><h3>{{ v.position or v.organization }}</h3><span class="dates">{{ dates(v.startDate, v.endDate) }}</span></div>
      <p class="sub">{% if v.position %}<span>{{ v.organization }}</span>{% endif %}{% if v.url %}<span>{{ link(v.url) }}</span>{% endif %}</p>
      {% if v.summary %}<div class="md">{{ v.summary | markdown }}</div>{% endif %}
      {% if v.highlights %}<ul class="bullets">{% for h in v.highlights %}<li>{{ h | markdown }}</li>{% endfor %}</ul>{% endif %}
    </article>
    {% endfor %}
  </section>
  {% endif %}

  {% if cv.sideProjects %}
  <section>
    <h2>Side projects</h2>
    {% for p in cv.sideProjects %}
    <article class="entry">
      <div class="row"><h3>{{ p.name }}</h3><span class="dates">{{ dates(p.startDate, p.endDate) if p.startDate or p.endDate }}</span></div>
      {% if p.url %}<p class="sub">{{ link(p.url) }}</p>{% endif %}
      {% if p.description %}<div class="md">{{ p.description | markdown }}</div>{% endif %}
      {{ tags(p.keywords) }}
    </article>
    {% endfor %}
  </section>
  {% endif %}

  {% if cv.awards %}
  <section>
    <h2>Awards</h2>
    {% for a in cv.awards %}
    <article class="entry">
      <div class="row"><h3>{{ a.title }}</h3><span class="dates">{{ a.date | format_date("%b %Y") }}</span></div>
      {% if a.awarder %}<p class="sub"><span>{{ a.awarder }}</span></p>{% endif %}
      {% if a.summary %}<div class="md">{{ a.summary | markdown }}</div>{% endif %}
    </article>
    {% endfor %}
  </section>
  {% endif %}

  {% if cv.certificates %}
  <section>
    <h2>Certificates</h2>
    {% for c in cv.certificates %}
    <article class="entry">
      <div class="row"><h3>{{ c.name }}</h3><span class="dates">{{ c.date | format_date("%b %Y") }}</span></div>
      <p class="sub">{% if c.issuer %}<span>{{ c.issuer }}</span>{% endif %}{% if c.url %}<span>{{ link(c.url) }}</span>{% endif %}</p>
    </article>
    {% endfor %}
  </section>
  {% endif %}

  {% if cv.publications %}
  <section>
    <h2>Publications</h2>
    {% for p in cv.publications %}
    <article class="entry">
      <div class="row"><h3>{{ p.name }}</h3><span class="dates">{{ p.releaseDate | format_date("%b %Y") }}</span></div>
      <p class="sub">{% if p.publisher %}<span>{{ p.publisher }}</span>{% endif %}{% if p.url %}<span>{{ link(p.url) }}</span>{% endif %}</p>
      {% if p.summary %}<div class="md">{{ p.summary | markdown }}</div>{% endif %}
    </article>
    {% endfor %}
  </section>
  {% endif %}

  {% if cv.languages %}
  <section>
    <h2>Languages</h2>
    <dl class="pairs">
      {% for l in cv.languages %}<div class="entry"><dt>{{ l.language }}</dt><dd>{{ l.fluency }}{% if l.summary %}<span class="md">{{ l.summary | markdown }}</span>{% endif %}</dd></div>{% endfor %}
    </dl>
  </section>
  {% endif %}

  {% if cv.interests %}
  <section>
    <h2>Interests</h2>
    <dl class="pairs">
      {% for i in cv.interests %}<div class="entry"><dt>{{ i.name }}</dt><dd>{{ (i.keywords or []) | join(" · ") }}</dd></div>{% endfor %}
    </dl>
  </section>
  {% endif %}

  {% if cv.references %}
  <section>
    <h2>References</h2>
    {% for r in cv.references %}
    <blockquote class="entry"><div class="md">{{ r.reference | markdown }}</div><footer>{{ r.name }}</footer></blockquote>
    {% endfor %}
  </section>
  {% endif %}
</main>
</body>
</html>
```

### style.css

```css
@page { size: A4; margin: 14mm 14mm; }

:root {
  --ink: #1c222b;
  --muted: #5d6673;
  --accent: #23405f;
  --rule: #d6dbe1;
  --serif: Charter, "Iowan Old Style", "Palatino Linotype", Palatino, Georgia, serif;
  --sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
}

* { box-sizing: border-box; }
html { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
body { margin: 0; color: var(--ink); font: 9.5pt/1.45 var(--sans); background: #fff; }
a { color: inherit; text-decoration: none; }

/* On screen the page is drawn as an A4 sheet; in print @page supplies the margin. */
.cv { width: 210mm; margin: 0 auto; padding: 14mm; }
@media print { .cv { width: auto; padding: 0; } }

.masthead { display: flex; justify-content: space-between; align-items: flex-start; gap: 8mm; padding-bottom: 4mm; border-bottom: 1.5pt solid var(--accent); }
h1 { margin: 0; font: 600 25pt/1.1 var(--serif); letter-spacing: -0.01em; color: var(--accent); }
.label { margin: 1.5mm 0 0; font-size: 11pt; color: var(--muted); }
.contact { display: flex; flex-wrap: wrap; gap: 0 4mm; margin: 3mm 0 0; padding: 0; list-style: none; font-size: 8.5pt; color: var(--muted); }
.photo { width: 26mm; height: 26mm; border-radius: 50%; object-fit: cover; flex: none; }

.summary { margin-top: 4mm; font-size: 10pt; }

section { margin-top: 5.5mm; }
h2 { margin: 0 0 2.5mm; padding-bottom: 1mm; border-bottom: 0.5pt solid var(--rule); font: 700 8pt/1.2 var(--sans); letter-spacing: 0.14em; text-transform: uppercase; color: var(--accent); }

.entry { break-inside: avoid; margin-bottom: 3.5mm; }
.entry:last-child { margin-bottom: 0; }
.row { display: flex; justify-content: space-between; align-items: baseline; gap: 6mm; }
h3 { margin: 0; font: 600 10.5pt/1.3 var(--sans); }
.dates { flex: none; font-size: 8.5pt; color: var(--muted); font-variant-numeric: tabular-nums; }
.sub { margin: 0; color: var(--muted); font-style: italic; }
.sub span + span::before { content: " · "; }
.sub:empty { display: none; }
.muted { margin: 1mm 0 0; color: var(--muted); font-size: 8.5pt; }

.md p { margin: 1mm 0 0; }
.md p:first-child { margin-top: 1mm; }
.bullets { margin: 1mm 0 0; padding-left: 4.5mm; }
.bullets li { margin: 0.4mm 0; padding-left: 0.5mm; }
.bullets li p { margin: 0; }
.bullets li::marker { color: var(--accent); }

.tags { display: flex; flex-wrap: wrap; gap: 1mm 1.6mm; margin: 1.5mm 0 0; }
.tags span { padding: 0 1.8mm; border: 0.5pt solid var(--rule); border-radius: 2mm; font-size: 8pt; color: var(--muted); }

.pairs { margin: 0; }
.pairs .entry { display: grid; grid-template-columns: 32mm 1fr; gap: 0 4mm; margin-bottom: 1.2mm; }
.pairs dt { font-weight: 600; }
.pairs dt small { font-weight: 400; color: var(--muted); }
.pairs dd { margin: 0; }
.pairs dd .md p { margin: 0; color: var(--muted); }

blockquote { margin: 0 0 3.5mm; padding-left: 3.5mm; border-left: 1.5pt solid var(--rule); }
blockquote footer { margin-top: 0.8mm; font-weight: 600; font-size: 8.5pt; color: var(--muted); }
```
