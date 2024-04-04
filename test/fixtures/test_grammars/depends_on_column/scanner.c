#include "tree_sitter/parser.h"

// STATELESS
void *tree_sitter_depends_on_column_external_scanner_create() { return NULL; }
void tree_sitter_depends_on_column_external_scanner_destroy(void *payload) {}
unsigned tree_sitter_depends_on_column_external_scanner_serialize(void *payload,
                                                                  char *buffer) {
  return 0;
}
void tree_sitter_depends_on_column_external_scanner_deserialize(void *payload,
	                                                            const char *buffer,
																unsigned length) {}
enum TokenType { ODD_COLUMN, EVEN_COLUMN };
bool tree_sitter_depends_on_column_external_scanner_scan(void *payload,
	                                                     TSLexer *lexer,
														 const bool *valid_symbols) {
  lexer->result_symbol =
      lexer->get_column(lexer) % 2 ? ODD_COLUMN : EVEN_COLUMN;
  return true;
}
