<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?><?schematron-phases phaseids="BANNER STRUCTURECHECK"?><!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00499">
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="ruleText">
        [ISM-ID-00499][Error] If a document has @ism:complieswith="USA-CUI" or "USA-CUI-ONLY", 
        then it must contain @ism:cuiControlledBy.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="codeDesc">
        If a document has @ism:complieswith="USA-CUI" or "USA-CUI-ONLY", then it must contain @ism:cuiControlledBy.
    </sch:p>
    <sch:rule id="ISM-ID-00499-R1"
             context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and (generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]">
        <sch:assert test="@ism:cuiControlledBy" flag="error" role="error">
            [ISM-ID-00499][Error] If a document has @ism:complieswith="USA-CUI" or "USA-CUI-ONLY", 
            then it must contain @ism:cuiControlledBy.
        </sch:assert>
    </sch:rule>
</sch:pattern>
