if(ENABLE_CJSON_TEST)
    add_library(unity STATIC unity/src/unity.c)

    # Disable -Werror for Unity
    if (FLAG_SUPPORTED_Werror)
        if ("${CMAKE_VERSION}" VERSION_LESS "2.8.12")
            set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wno-error")
        else()
            target_compile_options(unity PRIVATE "-Wno-error")
        endif()
    endif()
    # Disable -fvisibility=hidden for Unity
    if (FLAG_SUPPORTED_fvisibilityhidden)
        if ("${CMAKE_VERSION}" VERSION_LESS "2.8.12")
            set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fvisibility=default")
        else()
            target_compile_options(unity PRIVATE "-fvisibility=default")
        endif()
    endif()
    # Disable -fsanitize=float-divide-by-zero for Unity (GCC bug on x86 https://gcc.gnu.org/bugzilla/show_bug.cgi?id=80097)
    if (FLAG_SUPPORTED_fsanitizefloatdividebyzero AND (CMAKE_C_COMPILER_ID STREQUAL "GNU"))
        if ("${CMAKE_VERSION}" VERSION_LESS "2.8.12")
            set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fno-sanitize=float-divide-by-zero")
        else()
            target_compile_options(unity PRIVATE "-fno-sanitize=float-divide-by-zero")
        endif()
    endif()
    # Disable -Wswitch-enum for Unity
    if (FLAG_SUPPORTED_Wswitchenum)
        if ("${CMAKE_VERSION}" VERSION_LESS "2.8.12")
            set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wno-switch-enum")
        else()
            target_compile_options(unity PRIVATE "-Wno-switch-enum")
        endif()
    endif()

    #copy test files
    file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/inputs")
    file(GLOB test_files "inputs/*")
    file(COPY ${test_files} DESTINATION "${CMAKE_CURRENT_BINARY_DIR}/inputs/")

    if(ENABLE_CJSON_RUST)
        # These tests include cJSON.c to call static helpers. The Rust port
        # only exposes the public C ABI, so they are skipped here and covered
        # by rust/cjson-core unit tests instead.
        set(unity_tests
            parse_examples
            parse_with_opts
            compare_tests
            cjson_add
            readme_examples
            minify_tests
        )
    else()
        set(unity_tests
            parse_examples
            parse_number
            parse_hex4
            parse_string
            parse_array
            parse_object
            parse_value
            print_string
            print_number
            print_array
            print_object
            print_value
            misc_tests
            parse_with_opts
            compare_tests
            cjson_add
            readme_examples
            minify_tests
        )
    endif()

    option(ENABLE_VALGRIND OFF "Enable the valgrind memory checker for the tests.")
    if (ENABLE_VALGRIND)
        find_program(MEMORYCHECK_COMMAND valgrind)
        if ("${MEMORYCHECK_COMMAND}" MATCHES "MEMORYCHECK_COMMAND-NOTFOUND")
            message(WARNING "Valgrind couldn't be found.")
            unset(MEMORYCHECK_COMMAND)
        else()
            set(MEMORYCHECK_COMMAND_OPTIONS --trace-children=yes --leak-check=full --error-exitcode=1 --suppressions=${CMAKE_CURRENT_SOURCE_DIR}/../valgrind.supp)
        endif()
    endif()

    foreach(unity_test ${unity_tests})
        add_executable("${unity_test}" "${unity_test}.c")
        if("${CMAKE_C_COMPILER_ID}" STREQUAL "MSVC")
            target_sources(${unity_test} PRIVATE unity_setup.c)
        endif()
        if(ENABLE_CJSON_RUST)
            target_compile_definitions("${unity_test}" PRIVATE CJSON_RUST_PORT)
            target_include_directories("${unity_test}" PRIVATE "${CMAKE_SOURCE_DIR}")
        endif()
        target_link_libraries("${unity_test}" "${CJSON_LIB}" unity)
        if(MEMORYCHECK_COMMAND)
            add_test(NAME "${unity_test}"
                COMMAND "${MEMORYCHECK_COMMAND}" ${MEMORYCHECK_COMMAND_OPTIONS} "${CMAKE_CURRENT_BINARY_DIR}/${unity_test}")
        else()
            add_test(NAME "${unity_test}"
                COMMAND "./${unity_test}")
        endif()
    endforeach()

    add_dependencies(check ${unity_tests})

    if (ENABLE_CJSON_UTILS)
        #copy test files
        file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/json-patch-tests")
        file(GLOB test_files "json-patch-tests/*")
        file(COPY ${test_files} DESTINATION "${CMAKE_CURRENT_BINARY_DIR}/json-patch-tests/")

        set (cjson_utils_tests
            json_patch_tests
            old_utils_tests
            misc_utils_tests)

        foreach (cjson_utils_test ${cjson_utils_tests})
            add_executable("${cjson_utils_test}" "${cjson_utils_test}.c")
            if(ENABLE_CJSON_RUST)
                target_compile_definitions("${cjson_utils_test}" PRIVATE CJSON_RUST_PORT)
                target_include_directories("${cjson_utils_test}" PRIVATE "${CMAKE_SOURCE_DIR}")
            endif()
            target_link_libraries("${cjson_utils_test}" "${CJSON_LIB}" "${CJSON_UTILS_LIB}" unity)
            if("${CMAKE_C_COMPILER_ID}" STREQUAL "MSVC")
                target_sources(${cjson_utils_test} PRIVATE unity_setup.c)
            endif()
            if(MEMORYCHECK_COMMAND)
                add_test(NAME "${cjson_utils_test}"
                    COMMAND "${MEMORYCHECK_COMMAND}" ${MEMORYCHECK_COMMAND_OPTIONS} "${CMAKE_CURRENT_BINARY_DIR}/${cjson_utils_test}")
            else()
                add_test(NAME "${cjson_utils_test}"
                    COMMAND "./${cjson_utils_test}")
            endif()
        endforeach()

        add_dependencies(check ${cjson_utils_tests})
    endif()
endif()
