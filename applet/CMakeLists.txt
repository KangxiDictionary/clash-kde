add_definitions(-DTRANSLATION_DOMAIN=\"plasma_applet_org.kde.plasma.clashkde\")

plasma_add_applet(org.kde.plasma.clashkde
    QML_SOURCES
        main.qml
        config.qml
        ConfigBehavior.qml
    CPP_SOURCES
        clashcalls.cpp
    RESOURCES
        main.xml
    GENERATE_APPLET_CLASS
)

target_sources(org.kde.plasma.clashkde PRIVATE clashcalls.h)

target_link_libraries(org.kde.plasma.clashkde PRIVATE
    KF6::I18n
    Qt6::DBus
    Qt6::Gui
)

ki18n_install(po)
