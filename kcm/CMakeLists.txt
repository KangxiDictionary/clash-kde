cmake_minimum_required(VERSION 3.16)

project(kcm_clashkde VERSION 1.0.3)

set(QT_MIN_VERSION "6.6.0")
set(KF6_MIN_VERSION "6.0.0")

find_package(ECM ${KF6_MIN_VERSION} REQUIRED NO_MODULE)
set(CMAKE_MODULE_PATH ${ECM_MODULE_PATH} ${CMAKE_MODULE_PATH})

include(KDEInstallDirs)
include(KDECMakeSettings)
include(KDECompilerSettings NO_POLICY_SCOPE)

find_package(Qt6 ${QT_MIN_VERSION} REQUIRED COMPONENTS Core Quick Gui Qml DBus Network)
find_package(KF6 ${KF6_MIN_VERSION} REQUIRED COMPONENTS CoreAddons I18n KCMUtils)
find_package(KF6KirigamiAddons REQUIRED)
find_package(Plasma REQUIRED)
find_package(ICU REQUIRED COMPONENTS uc)

kcmutils_add_qml_kcm(kcm_clashkde SOURCES
    kcm.cpp
    clashclient.cpp
)

target_link_libraries(kcm_clashkde PRIVATE
    Qt6::Core
    Qt6::Quick
    Qt6::DBus
    Qt6::Network
    KF6::CoreAddons
    KF6::I18n
    KF6::KCMUtils
    KF6::KCMUtilsQuick
    ICU::uc
)

ki18n_install(po)

add_subdirectory(${CMAKE_CURRENT_SOURCE_DIR}/../applet ${CMAKE_CURRENT_BINARY_DIR}/applet)
