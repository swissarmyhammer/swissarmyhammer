mod entity_extractor;
mod languages;

use std::cell::RefCell;
use std::collections::HashMap;

use crate::model::entity::SemanticEntity;
use crate::parser::plugin::SemanticParserPlugin;
use entity_extractor::extract_entities;
use languages::{get_all_code_extensions, get_language_config};

pub struct CodeParserPlugin;

// Thread-local parser cache: one Parser per language per thread.
// Avoids creating a new Parser for every file during parallel graph builds.
thread_local! {
    static PARSER_CACHE: RefCell<HashMap<&'static str, tree_sitter::Parser>> = RefCell::new(HashMap::new());
}

impl SemanticParserPlugin for CodeParserPlugin {
    fn id(&self) -> &str {
        "code"
    }

    fn extensions(&self) -> &[&str] {
        get_all_code_extensions()
    }

    fn extract_entities(&self, content: &str, file_path: &str) -> Vec<SemanticEntity> {
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_lowercase()))
            .unwrap_or_default();

        let config = match get_language_config(&ext) {
            Some(c) => c,
            None => return Vec::new(),
        };

        let language = match (config.get_language)() {
            Some(lang) => lang,
            None => return Vec::new(),
        };

        PARSER_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let parser = cache.entry(config.id).or_insert_with(|| {
                let mut p = tree_sitter::Parser::new();
                let _ = p.set_language(&language);
                p
            });

            let tree = match parser.parse(content.as_bytes(), None) {
                Some(t) => t,
                None => return Vec::new(),
            };

            extract_entities(&tree, file_path, config, content)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_entity_extraction() {
        let code = r#"
package com.example;

import java.util.List;

public class UserService {
    private String name;

    public UserService(String name) {
        this.name = name;
    }

    public List<User> getUsers() {
        return db.findAll();
    }

    public void createUser(User user) {
        db.save(user);
    }
}

interface Repository<T> {
    T findById(String id);
    List<T> findAll();
}

enum Status {
    ACTIVE,
    INACTIVE,
    DELETED
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "UserService.java");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Java entities: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"UserService"),
            "Should find class UserService, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Repository"),
            "Should find interface Repository, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Status"),
            "Should find enum Status, got: {:?}",
            names
        );
    }

    #[test]
    fn test_java_nested_methods() {
        let code = r#"
public class Calculator {
    public int add(int a, int b) {
        return a + b;
    }

    public int subtract(int a, int b) {
        return a - b;
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "Calculator.java");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "Java nested: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Calculator"),
            "Should find Calculator class"
        );
        assert!(
            names.contains(&"add"),
            "Should find add method, got: {:?}",
            names
        );
        assert!(
            names.contains(&"subtract"),
            "Should find subtract method, got: {:?}",
            names
        );

        // Methods should have Calculator as parent
        let add = entities.iter().find(|e| e.name == "add").unwrap();
        assert!(add.parent_id.is_some(), "add should have parent_id");
    }

    #[test]
    fn test_c_entity_extraction() {
        let code = r#"
#include <stdio.h>

struct Point {
    int x;
    int y;
};

enum Color {
    RED,
    GREEN,
    BLUE
};

typedef struct {
    char name[50];
    int age;
} Person;

void greet(const char* name) {
    printf("Hello, %s!\n", name);
}

int add(int a, int b) {
    return a + b;
}

int main() {
    greet("world");
    return 0;
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "main.c");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "C entities: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"greet"),
            "Should find greet function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"add"),
            "Should find add function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"main"),
            "Should find main function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Point"),
            "Should find Point struct, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Color"),
            "Should find Color enum, got: {:?}",
            names
        );
    }

    #[test]
    fn test_cpp_entity_extraction() {
        let code = "namespace math {\nclass Vector3 {\npublic:\n    float length() const { return 0; }\n};\n}\nvoid greet() {}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "main.cpp");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"math"), "got: {:?}", names);
        assert!(names.contains(&"Vector3"), "got: {:?}", names);
        assert!(names.contains(&"greet"), "got: {:?}", names);
    }

    #[test]
    fn test_ruby_entity_extraction() {
        let code = "module Auth\n  class User\n    def greet\n      \"hi\"\n    end\n  end\nend\ndef helper(x)\n  x * 2\nend\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "auth.rb");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Auth"), "got: {:?}", names);
        assert!(names.contains(&"User"), "got: {:?}", names);
        assert!(names.contains(&"helper"), "got: {:?}", names);
    }

    #[test]
    fn test_csharp_entity_extraction() {
        let code = "namespace MyApp {\npublic class User {\n    public string GetName() { return \"\"; }\n}\npublic enum Role { Admin, User }\n}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "Models.cs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"MyApp"), "got: {:?}", names);
        assert!(names.contains(&"User"), "got: {:?}", names);
        assert!(names.contains(&"Role"), "got: {:?}", names);
    }

    #[test]
    fn test_swift_entity_extraction() {
        let code = r#"
import Foundation

class UserService {
    var name: String

    init(name: String) {
        self.name = name
    }

    func getUsers() -> [User] {
        return db.findAll()
    }
}

struct Point {
    var x: Double
    var y: Double
}

enum Status {
    case active
    case inactive
    case deleted
}

protocol Repository {
    associatedtype Item
    func findById(id: String) -> Item?
    func findAll() -> [Item]
}

func helper(x: Int) -> Int {
    return x * 2
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "UserService.swift");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "Swift entities: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"UserService"),
            "Should find class UserService, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Point"),
            "Should find struct Point, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Status"),
            "Should find enum Status, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Repository"),
            "Should find protocol Repository, got: {:?}",
            names
        );
        assert!(
            names.contains(&"helper"),
            "Should find function helper, got: {:?}",
            names
        );
    }

    #[test]
    fn test_elixir_entity_extraction() {
        let code = r#"
defmodule MyApp.Accounts do
  def create_user(attrs) do
    %User{}
    |> User.changeset(attrs)
    |> Repo.insert()
  end

  defp validate(attrs) do
    # private helper
    :ok
  end

  defmacro is_admin(user) do
    quote do
      unquote(user).role == :admin
    end
  end

  defguard is_positive(x) when is_integer(x) and x > 0
end

defprotocol Printable do
  def to_string(data)
end

defimpl Printable, for: Integer do
  def to_string(i), do: Integer.to_string(i)
end
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "accounts.ex");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Elixir entities: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"MyApp.Accounts"),
            "Should find module, got: {:?}",
            names
        );
        assert!(
            names.contains(&"create_user"),
            "Should find def, got: {:?}",
            names
        );
        assert!(
            names.contains(&"validate"),
            "Should find defp, got: {:?}",
            names
        );
        assert!(
            names.contains(&"is_admin"),
            "Should find defmacro, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Printable"),
            "Should find defprotocol, got: {:?}",
            names
        );

        // Verify nesting: create_user should have MyApp.Accounts as parent
        let create_user = entities.iter().find(|e| e.name == "create_user").unwrap();
        assert!(
            create_user.parent_id.is_some(),
            "create_user should be nested under module"
        );
    }

    #[test]
    fn test_bash_entity_extraction() {
        let code = r#"#!/bin/bash

greet() {
    echo "Hello, $1!"
}

function deploy {
    echo "deploying..."
}

# not a function
echo "main script"
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "deploy.sh");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Bash entities: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"greet"),
            "Should find greet(), got: {:?}",
            names
        );
        assert!(
            names.contains(&"deploy"),
            "Should find function deploy, got: {:?}",
            names
        );
        assert_eq!(
            entities.len(),
            2,
            "Should only find functions, got: {:?}",
            names
        );
    }

    #[test]
    fn test_typescript_entity_extraction() {
        // Existing language should still work
        let code = r#"
export function hello(): string {
    return "hello";
}

export class Greeter {
    greet(name: string): string {
        return `Hello, ${name}!`;
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "test.ts");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"hello"), "Should find hello function");
        assert!(names.contains(&"Greeter"), "Should find Greeter class");
    }

    #[test]
    fn test_typescript_class_with_methods() {
        // Tests class/method extraction and nested parent_id assignment
        let code = r#"
class Animal {
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    speak(): string {
        return `${this.name} makes a sound.`;
    }

    static create(name: string): Animal {
        return new Animal(name);
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "animal.ts");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "TS class+methods: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Animal"),
            "Should find Animal class, got: {:?}",
            names
        );
        assert!(
            names.contains(&"speak"),
            "Should find speak method, got: {:?}",
            names
        );

        // speak should have Animal as parent
        let speak = entities.iter().find(|e| e.name == "speak").unwrap();
        assert!(
            speak.parent_id.is_some(),
            "speak method should have a parent_id"
        );
    }

    #[test]
    fn test_typescript_interface_extraction() {
        // Tests interface declaration and its body members
        let code = r#"
interface Shape {
    area(): number;
    perimeter(): number;
    color: string;
}

interface Drawable extends Shape {
    draw(): void;
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "shapes.ts");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "TS interface: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Shape"),
            "Should find Shape interface, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Drawable"),
            "Should find Drawable interface, got: {:?}",
            names
        );

        let shape = entities.iter().find(|e| e.name == "Shape").unwrap();
        assert_eq!(
            shape.entity_type, "interface",
            "Shape should be an interface"
        );
    }

    #[test]
    fn test_rust_struct_and_trait_extraction() {
        // Tests Rust struct_item, trait_item, impl_item, and nested function_item
        let code = r#"
pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub trait Shape {
    fn area(&self) -> f64;
    fn perimeter(&self) -> f64;
    fn name(&self) -> &str {
        "shape"
    }
}

pub struct Circle {
    pub center: Point,
    pub radius: f64,
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }

    fn perimeter(&self) -> f64 {
        2.0 * std::f64::consts::PI * self.radius
    }
}

impl Circle {
    pub fn new(x: f64, y: f64, radius: f64) -> Self {
        Circle {
            center: Point { x, y },
            radius,
        }
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "geometry.rs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Rust struct+trait: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Point"),
            "Should find Point struct, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Shape"),
            "Should find Shape trait, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Circle"),
            "Should find Circle struct, got: {:?}",
            names
        );

        // Verify entity types
        let point = entities.iter().find(|e| e.name == "Point").unwrap();
        assert_eq!(point.entity_type, "struct", "Point should be a struct");

        let shape = entities.iter().find(|e| e.name == "Shape").unwrap();
        assert_eq!(shape.entity_type, "trait", "Shape should be a trait");
    }

    #[test]
    fn test_rust_impl_nested_methods() {
        // Tests that methods inside impl blocks have parent_id set
        let code = r#"
pub struct Counter {
    count: u32,
}

impl Counter {
    pub fn new() -> Self {
        Counter { count: 0 }
    }

    pub fn increment(&mut self) {
        self.count += 1;
    }

    pub fn value(&self) -> u32 {
        self.count
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "counter.rs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "Rust impl methods: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Counter"),
            "Should find Counter struct, got: {:?}",
            names
        );
        assert!(
            names.contains(&"new"),
            "Should find new function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"increment"),
            "Should find increment function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"value"),
            "Should find value function, got: {:?}",
            names
        );

        // Methods inside impl should have parent_id
        let new_fn = entities.iter().find(|e| e.name == "new").unwrap();
        assert!(
            new_fn.parent_id.is_some(),
            "new function should have parent_id (impl block)"
        );
    }

    #[test]
    fn test_python_class_with_methods() {
        // Tests Python class_definition containing method_definition (function_definition in block)
        let code = r#"
class Animal:
    def __init__(self, name: str):
        self.name = name

    def speak(self) -> str:
        return f"{self.name} makes a sound"

    def __repr__(self) -> str:
        return f"Animal({self.name!r})"


class Dog(Animal):
    def speak(self) -> str:
        return f"{self.name} barks"

    @staticmethod
    def species() -> str:
        return "Canis lupus familiaris"


def standalone_function(x: int) -> int:
    return x * 2
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "animals.py");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "Python class+methods: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Animal"),
            "Should find Animal class, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Dog"),
            "Should find Dog class, got: {:?}",
            names
        );
        assert!(
            names.contains(&"standalone_function"),
            "Should find standalone_function, got: {:?}",
            names
        );

        // Verify class type
        let animal = entities.iter().find(|e| e.name == "Animal").unwrap();
        assert_eq!(animal.entity_type, "class", "Animal should be a class");

        // Methods should be nested
        let speak_methods: Vec<_> = entities.iter().filter(|e| e.name == "speak").collect();
        assert!(
            !speak_methods.is_empty(),
            "Should find speak methods, got: {:?}",
            names
        );

        // At least one speak should have a parent
        let has_parent = speak_methods.iter().any(|e| e.parent_id.is_some());
        assert!(
            has_parent,
            "speak methods should have parent_id (the class)"
        );
    }

    #[test]
    fn test_python_decorated_class() {
        // Tests decorated_definition for class (map_decorated_type returns "class")
        let code = r#"
import dataclasses

@dataclasses.dataclass
class Config:
    host: str
    port: int
    debug: bool = False

@staticmethod
def helper():
    pass
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "config.py");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Python decorated: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Config"),
            "Should find Config class, got: {:?}",
            names
        );

        let config = entities.iter().find(|e| e.name == "Config").unwrap();
        // decorated_definition with class_definition inside → should map to "class"
        assert_eq!(
            config.entity_type, "class",
            "Decorated class should have entity_type 'class'"
        );
    }

    #[test]
    fn test_go_method_and_type_extraction() {
        // Tests Go method_declaration and function_declaration extraction.
        // Note: Go tree-sitter represents `type Rectangle struct { ... }` as a
        // type_declaration containing a type_spec, so the name is not at the
        // type_declaration level directly. Methods (func with receiver) use
        // method_declaration which does have a name field.
        let code = r#"
package main

import "fmt"

type Rectangle struct {
    Width  float64
    Height float64
}

type Circle struct {
    Radius float64
}

func (r Rectangle) Area() float64 {
    return r.Width * r.Height
}

func (r Rectangle) Perimeter() float64 {
    return 2 * (r.Width + r.Height)
}

func (c Circle) Area() float64 {
    return 3.14159 * c.Radius * c.Radius
}

func main() {
    r := Rectangle{Width: 3, Height: 4}
    fmt.Println(r.Area())
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "shapes.go");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let _types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Go method+type: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type))
                .collect::<Vec<_>>()
        );

        // Go methods (with receiver) should be found
        let area_methods: Vec<_> = entities.iter().filter(|e| e.name == "Area").collect();
        assert!(
            !area_methods.is_empty(),
            "Should find Area method declarations, got: {:?}",
            names
        );

        // Verify method entity type
        let area = area_methods[0];
        assert_eq!(area.entity_type, "method", "Area should be a method");

        // Regular function should be found
        assert!(
            names.contains(&"main"),
            "Should find main function, got: {:?}",
            names
        );

        let main_fn = entities.iter().find(|e| e.name == "main").unwrap();
        assert_eq!(main_fn.entity_type, "function", "main should be a function");
    }

    #[test]
    fn test_php_class_and_trait_extraction() {
        // Tests PHP class_declaration, trait_declaration, method_declaration, interface_declaration
        let code = r#"<?php

namespace App\Models;

interface Printable {
    public function toString(): string;
}

trait Timestampable {
    private \DateTime $createdAt;

    public function getCreatedAt(): \DateTime {
        return $this->createdAt;
    }

    public function setCreatedAt(\DateTime $dt): void {
        $this->createdAt = $dt;
    }
}

class User implements Printable {
    use Timestampable;

    private string $name;

    public function __construct(string $name) {
        $this->name = $name;
    }

    public function toString(): string {
        return $this->name;
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "User.php");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "PHP class+trait: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Printable"),
            "Should find Printable interface, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Timestampable"),
            "Should find Timestampable trait, got: {:?}",
            names
        );
        assert!(
            names.contains(&"User"),
            "Should find User class, got: {:?}",
            names
        );

        // Verify entity types
        let printable = entities.iter().find(|e| e.name == "Printable").unwrap();
        assert_eq!(
            printable.entity_type, "interface",
            "Printable should be an interface"
        );

        let timestampable = entities.iter().find(|e| e.name == "Timestampable").unwrap();
        assert_eq!(
            timestampable.entity_type, "trait",
            "Timestampable should be a trait"
        );
    }

    #[test]
    fn test_javascript_class_with_methods() {
        // Tests JS class with method_definition inside class_body
        let code = r#"
class EventEmitter {
    #listeners = new Map();

    on(event, listener) {
        if (!this.#listeners.has(event)) {
            this.#listeners.set(event, []);
        }
        this.#listeners.get(event).push(listener);
        return this;
    }

    emit(event, ...args) {
        const listeners = this.#listeners.get(event) || [];
        listeners.forEach(fn => fn(...args));
        return this;
    }

    off(event, listener) {
        const arr = this.#listeners.get(event);
        if (arr) {
            this.#listeners.set(event, arr.filter(l => l !== listener));
        }
        return this;
    }
}

function createEmitter() {
    return new EventEmitter();
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "events.js");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "JS class+methods: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"EventEmitter"),
            "Should find EventEmitter class, got: {:?}",
            names
        );
        assert!(
            names.contains(&"on"),
            "Should find on method, got: {:?}",
            names
        );
        assert!(
            names.contains(&"emit"),
            "Should find emit method, got: {:?}",
            names
        );
        assert!(
            names.contains(&"createEmitter"),
            "Should find createEmitter function, got: {:?}",
            names
        );

        // Methods should have EventEmitter as parent
        let on_method = entities.iter().find(|e| e.name == "on").unwrap();
        assert!(
            on_method.parent_id.is_some(),
            "on method should have parent_id"
        );
    }

    #[test]
    fn test_rust_trait_with_default_methods() {
        // Tests trait_item with methods inside declaration_list
        let code = r#"
pub trait Greet {
    fn name(&self) -> &str;

    fn greeting(&self) -> String {
        format!("Hello, {}!", self.name())
    }

    fn farewell(&self) -> String {
        format!("Goodbye, {}!", self.name())
    }
}

pub struct Person {
    pub name: String,
}

impl Greet for Person {
    fn name(&self) -> &str {
        &self.name
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "greet.rs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "Rust trait methods: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Greet"),
            "Should find Greet trait, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Person"),
            "Should find Person struct, got: {:?}",
            names
        );

        let greet = entities.iter().find(|e| e.name == "Greet").unwrap();
        assert_eq!(greet.entity_type, "trait", "Greet should be a trait");

        // Methods inside trait should have parent_id
        let greeting = entities.iter().find(|e| e.name == "greeting");
        if let Some(g) = greeting {
            assert!(
                g.parent_id.is_some(),
                "greeting should have parent_id (trait)"
            );
        }
    }

    #[test]
    fn test_typescript_enum_extraction() {
        // Tests TypeScript enum_declaration
        let code = r#"
enum Direction {
    Up = "UP",
    Down = "DOWN",
    Left = "LEFT",
    Right = "RIGHT",
}

const enum Color {
    Red,
    Green,
    Blue,
}

type Point = {
    x: number;
    y: number;
};
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "types.ts");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "TS enum+type: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Direction"),
            "Should find Direction enum, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Point"),
            "Should find Point type alias, got: {:?}",
            names
        );

        let direction = entities.iter().find(|e| e.name == "Direction").unwrap();
        assert_eq!(direction.entity_type, "enum", "Direction should be an enum");

        let point = entities.iter().find(|e| e.name == "Point").unwrap();
        assert_eq!(point.entity_type, "type", "Point should be a type alias");
    }
}
