inject_index: "blog"

is this necessary? url_override: /\d{4}/(\d{1,2}/(\d{1,2}/)?)?

aargh. how to handle those generated pages in year/month/day that do not exist and that i don't want to include in the source explicitly one by one?

with this file somehow?

with a flag in the blog post page, to generate all parent directory indexes?

with a flag in this metadata, to match all pages of a specified group?

bind this as a special case to page dates? but date isn't special (yet), not even a field yet?

looking at parent directories and if they match might be okay enough - go through all pages before rendering, and add virtual pages linking to this source for each matching parent path. after having read all sources, find pages like this, with inject_index set, and read the url in the template, then find groups by that.
