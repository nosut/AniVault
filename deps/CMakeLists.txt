add_library(taiga-deps STATIC)

target_link_libraries(taiga-deps
	anitomy
	monolog
	nstd
	semaver
	taiga-config
	utf8proc
)

add_subdirectory(anitomy)
add_subdirectory(monolog)
add_subdirectory(nstd)
add_subdirectory(semaver)
add_subdirectory(utf8proc)
