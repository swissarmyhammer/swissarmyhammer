Partials are not rendering, and I can find no test of partial rendering.

As a reminder, and file in a ./prompts directory that ends in:

.md
.liquid.md
.liquid.markdown
.liquid

should be available as a partial to use with {% render %}

for example if I have:

~/.swissarmyhammer/prompts/partials/top.liquid.md

I should be able to use it in a prompt with

{% render "partials/top" %}

~/.swissarmyhammer/prompts/stuff.liquid.md

I should be able to use it in a prompt with

%{ render "stuff" }

Realtive path to the prompts folder, extensions removed.
