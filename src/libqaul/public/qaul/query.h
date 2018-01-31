
#include <glob.h>

typedef enum ql_limit {
  
  STARTS_WITH, ENDS_WITH, EQUALS,
  
  NEWER, OLDER,
};

typedef struct ql_query {
  char **name;
  enum ql_limit *name_limits;
  short names;

  unsigned int *time;
  enum ql_limit *time_limits;
  short times;

  size_t max_count;
};