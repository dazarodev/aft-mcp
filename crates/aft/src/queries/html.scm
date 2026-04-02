;; regular elements
(element
  (start_tag
    (tag_name) @tag.name)) @tag.def

;; script elements
(script_element
  (start_tag
    (tag_name) @script.name)) @script.def

;; style elements
(style_element
  (start_tag
    (tag_name) @style.name)) @style.def
