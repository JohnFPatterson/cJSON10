/* Compare C header layout against the Rust #[repr(C)] struct. */
#include <stddef.h>
#include <stdio.h>
#include "../../../cJSON.h"

int main(void)
{
    printf("sizeof=%zu\n", sizeof(cJSON));
    printf("next=%zu\n", offsetof(cJSON, next));
    printf("prev=%zu\n", offsetof(cJSON, prev));
    printf("child=%zu\n", offsetof(cJSON, child));
    printf("type=%zu\n", offsetof(cJSON, type));
    printf("valuestring=%zu\n", offsetof(cJSON, valuestring));
    printf("valueint=%zu\n", offsetof(cJSON, valueint));
    printf("valuedouble=%zu\n", offsetof(cJSON, valuedouble));
    printf("string=%zu\n", offsetof(cJSON, string));
    return 0;
}
