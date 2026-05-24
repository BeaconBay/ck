; Markdown Tree-sitter queries for chunk boundaries

; Headings and sections
(atx_heading) @module
(setext_heading) @module
(section) @module

; Block elements
(paragraph) @text
(fenced_code_block) @text
(indented_code_block) @text
(block_quote) @text
(list) @text
(list_item) @text
(thematic_break) @text
