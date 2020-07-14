rotulus static site generator
=============================

(i'd prefer the word compiler instead of generator but generator seems to be the globally accepted one.)

Rotulus is a roll that holds some papyrus that holds writing. (XXX TODO no, it's more)
Rotulus is dumb, it just is a support structure for your data, making it slightly easier to present it.
You just format the data with your templates.
To make that possible, everything necessary is passed as a context in to the templates, along with filtering utilities and such.

Rotulus is generic; no site-specific modifications in its code should be required.
Your data should be so simple that no more complex operations than data transformations need to be performed in the templates.
Rotulus provides those template filters and usual logic.

Supports reStructuredText markup for the source data, and Tera (Jinja2/Django style) templates.
If I change my mind, those are supposed to be easy enough to change.
No template code is allowed in the source data, because data should be tool-agnostic.
All templates are yours; rotulus does not provide any.

The core does not support much else than:

* rendering the data through templates
* arbitrary metadata embedded in the source data, passed to the templates
* groups of pages, so that templates can explicitly list all blog posts, categories, and such
* generating virtual urls for automatic listing pages not explicitly stated in the source, e.g., creating by-date hierarchies for blog posts
* draft pages, not included in the output unless you say so
* linking between the same page written in different languages
* scaling large source images for thumbs and links to originals (XXX: or in a makefile outside this?)
* semi-automatic pagination (XXX do I need this?)

Written in rust because i need(ed) to learn it, so don't expect professional quality.
Yet.
Not meant to be used by anyone else, just my personal toy.
For now.
