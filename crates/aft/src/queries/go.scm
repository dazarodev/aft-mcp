;; function declarations
(function_declaration
  name: (identifier) @fn.name) @fn.def

;; method declarations (with receiver)
(method_declaration
  name: (field_identifier) @method.name) @method.def

;; type declarations (struct and interface)
(type_declaration
  (type_spec
    name: (type_identifier) @type.name
    type: (_) @type.body)) @type.def
