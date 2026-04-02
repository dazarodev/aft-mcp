;; class declarations
(class_declaration
  name: (identifier) @class.name) @class.def

;; method declarations
(method_declaration
  name: (identifier) @method.name) @method.def

;; interface declarations
(interface_declaration
  name: (identifier) @interface.name) @interface.def

;; enum declarations
(enum_declaration
  name: (identifier) @enum.name) @enum.def

;; trigger declarations
(trigger_declaration
  name: (identifier) @trigger.name) @trigger.def
