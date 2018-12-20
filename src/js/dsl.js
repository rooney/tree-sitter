const UNICODE_ESCAPE_PATTERN = /\\u([0-9a-f]{4})/gi;
const DELIMITER_ESCAPE_PATTERN = /\\\//g;

function alias(rule, value) {
  const result = {
    type: "ALIAS",
    content: normalize(rule),
    named: false,
    value: null
  };

  switch (value.constructor) {
    case String:
      result.named = false;
      result.value = value;
      return result;
    case ReferenceError:
      result.named = true;
      result.value = value.symbol.name;
      return result;
    case Object:
      if (typeof value.type === 'string' && value.type === 'SYMBOL') {
        result.named = true;
        result.value = value.name;
        return result;
      }
  }

  throw new Error('Invalid alias value ' + value);
}

function blank() {
  return {
    type: "BLANK"
  };
}

function choice(...elements) {
  return {
    type: "CHOICE",
    members: elements.map(normalize)
  };
}

function optional(value) {
  return choice(value, blank());
}

function prec(number, rule) {
  if (rule == null) {
    rule = number;
    number = 0;
  }

  return {
    type: "PREC",
    value: number,
    content: normalize(rule)
  };
}

prec.left = function(number, rule) {
  if (rule == null) {
    rule = number;
    number = 0;
  }

  return {
    type: "PREC_LEFT",
    value: number,
    content: normalize(rule)
  };
}

prec.right = function(number, rule) {
  if (rule == null) {
    rule = number;
    number = 0;
  }

  return {
    type: "PREC_RIGHT",
    value: number,
    content: normalize(rule)
  };
}

prec.dynamic = function(number, rule) {
  return {
    type: "PREC_DYNAMIC",
    value: number,
    content: normalize(rule)
  };
}

function repeat(rule) {
  return {
    type: "REPEAT",
    content: normalize(rule)
  };
}

function repeat1(rule) {
  return {
    type: "REPEAT1",
    content: normalize(rule)
  };
}

function seq(...elements) {
  return {
    type: "SEQ",
    members: elements.map(normalize)
  };
}

function sym(name) {
  return {
    type: "SYMBOL",
    name: name
  };
}

function token(value) {
  return {
    type: "TOKEN",
    content: normalize(value)
  };
}

token.immediate = function(value) {
  return {
    type: "IMMEDIATE_TOKEN",
    content: normalize(value)
  };
}

function normalize(value) {

  if (typeof value == "undefined")
    throw new Error("Undefined symbol");

  switch (value.constructor) {
    case String:
      return {
        type: 'STRING',
        value
      };
    case RegExp:
      return {
          type: 'PATTERN',
          value: value.source
            .replace(
              DELIMITER_ESCAPE_PATTERN,
              '/'
            )
            .replace(
              UNICODE_ESCAPE_PATTERN,
              (match, group) => String.fromCharCode(parseInt(group, 16))
            )
      };
    case ReferenceError:
      throw value
    default:
      if (typeof value.type === 'string') {
        return value;
      } else {
        throw new TypeError("Invalid rule: " + value.toString());
      }
  }
}

function RuleBuilder(ruleMap) {
  return new Proxy({}, {
    get(target, propertyName) {
      const symbol = {
        type: 'SYMBOL',
        name: propertyName
      };

      if (!ruleMap || ruleMap.hasOwnProperty(propertyName)) {
        return symbol;
      } else {
        const error = new ReferenceError(`Undefined symbol '${propertyName}'`);
        error.symbol = symbol;
        return error;
      }
    }
  })
}

function grammar(baseGrammar, options) {
    if (!options) {
      options = baseGrammar;
      baseGrammar = {
        name: null,
        rules: {},
        extras: [normalize(/\s/)],
        conflicts: [],
        externals: [],
        inline: []
      };
    }

    let externals = baseGrammar.externals;
    if (options.externals) {
      if (typeof options.externals !== "function") {
        throw new Error("Grammar's 'externals' property must be a function.");
      }

      const externalsRuleBuilder = RuleBuilder(null)
      const externalRules = options.externals.call(externalsRuleBuilder, externalsRuleBuilder, baseGrammar.externals);

      if (!Array.isArray(externalRules)) {
        throw new Error("Grammar's 'externals' property must return an array of rules.");
      }

      externals = externalRules.map(normalize);
    }

    const ruleMap = {};
    for (const key in options.rules) {
      ruleMap[key] = true;
    }
    for (const key in baseGrammar.rules) {
      ruleMap[key] = true;
    }
    for (const external of externals) {
      if (typeof external.name === 'string') {
        ruleMap[external.name] = true;
      }
    }

    const ruleBuilder = RuleBuilder(ruleMap);

    const name = options.name;
    if (typeof name !== "string") {
      throw new Error("Grammar's 'name' property must be a string.");
    }

    if (!/^[a-zA-Z_]\w*$/.test(name)) {
      throw new Error("Grammar's 'name' property must not start with a digit and cannot contain non-word characters.");
    }

    let rules = Object.assign({}, baseGrammar.rules);
    if (options.rules) {
      if (typeof options.rules !== "object") {
        throw new Error("Grammar's 'rules' property must be an object.");
      }

      for (const ruleName in options.rules) {
        const ruleFn = options.rules[ruleName];
        if (typeof ruleFn !== "function") {
          throw new Error("Grammar rules must all be functions. '" + ruleName + "' rule is not.");
        }
        rules[ruleName] = normalize(ruleFn.call(ruleBuilder, ruleBuilder, baseGrammar.rules[ruleName]));
      }
    }

    let extras = baseGrammar.extras.slice();
    if (options.extras) {
      if (typeof options.extras !== "function") {
        throw new Error("Grammar's 'extras' property must be a function.");
      }

      extras = options.extras
        .call(ruleBuilder, ruleBuilder, baseGrammar.extras)
        .map(normalize);
    }

    let word = baseGrammar.word;
    if (options.word) {
      word = options.word.call(ruleBuilder, ruleBuilder).name;
      if (typeof word != 'string') {
        throw new Error("Grammar's 'word' property must be a named rule.");
      }
    }

    let conflicts = baseGrammar.conflicts;
    if (options.conflicts) {
      if (typeof options.conflicts !== "function") {
        throw new Error("Grammar's 'conflicts' property must be a function.");
      }

      const baseConflictRules = baseGrammar.conflicts.map(conflict => conflict.map(sym));
      const conflictRules = options.conflicts.call(ruleBuilder, ruleBuilder, baseConflictRules);

      if (!Array.isArray(conflictRules)) {
        throw new Error("Grammar's conflicts must be an array of arrays of rules.");
      }

      conflicts = conflictRules.map(conflictSet => {
        if (!Array.isArray(conflictSet)) {
          throw new Error("Grammar's conflicts must be an array of arrays of rules.");
        }

        return conflictSet.map(symbol => symbol.name);
      });
    }

    let inline = baseGrammar.inline;
    if (options.inline) {
      if (typeof options.inline !== "function") {
        throw new Error("Grammar's 'inline' property must be a function.");
      }

      const baseInlineRules = baseGrammar.inline.map(sym);
      const inlineRules = options.inline.call(ruleBuilder, ruleBuilder, baseInlineRules);

      if (!Array.isArray(inlineRules)) {
        throw new Error("Grammar's inline must be an array of rules.");
      }

      inline = inlineRules.map(symbol => symbol.name);
    }

    if (Object.keys(rules).length == 0) {
      throw new Error("Grammar must have at least one rule.");
    }

    return {name, word, rules, extras, conflicts, externals, inline};
  }

global.alias = alias;
global.blank = blank;
global.choice = choice;
global.optional = optional;
global.prec = prec;
global.repeat = repeat;
global.repeat1 = repeat1;
global.seq = seq;
global.sym = sym;
global.token = token;
global.grammar = grammar;
