;; function definitions (top-level and nested)
(function_definition
  name: (identifier) @fn.name) @fn.def

;; class definitions
(class_definition
  name: (identifier) @class.name) @class.def

;; decorated definitions (wraps function_definition or class_definition)
(decorated_definition
  (decorator) @dec.decorator) @dec.def
