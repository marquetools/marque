<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?><?schematron-phases phaseids="BANNER VALUECHECK"?><!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00498">
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="ruleText">
        [ISM-ID-00498][Error] If a document contains @ism:cuiControlledBy,
        then it must contain at least one of @ism:cuiBasic or @ism:cuiSpecified.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="codeDesc">
        If a document contains @ism:cuiControlledBy, then it must contain at least one of @ism:cuiBasic or @ism:cuiSpecified.
    </sch:p>
    <sch:rule id="ISM-ID-00498-R1"
             context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and (generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and @ism:cuiControlledBy]">
        <sch:assert test="@ism:cuiBasic or @ism:cuiSpecified" flag="error" role="error">
            [ISM-ID-00498][Error] If a document contains @ism:cuiControlledBy,
            then it must contain at least one of @ism:cuiBasic or @ism:cuiSpecified.
        </sch:assert>
    </sch:rule>
</sch:pattern>
