<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?><?schematron-phases phaseids="BANNER VALUECHECK"?><!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00500">
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="ruleText">
        [ISM-ID-00500][Error] If a document has @ism:cuiControlledBy, 
        then it must have ism:complieswith="USA-CUI" or "USA-CUI-ONLY".
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="codeDesc">
        If a document has @ism:cuiControlledBy, then it must have ism:complieswith="USA-CUI" or "USA-CUI-ONLY".
    </sch:p>
    <sch:rule id="ISM-ID-00500-R1"
             context="*[(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and @ism:cuiControlledBy]">
        <sch:assert test="contains(@ism:compliesWith,'USA-CUI') or @ism:compliesWith='USA-CUI-ONLY'"
                  flag="error"
                  role="error">
            [ISM-ID-00500][Error]  If a document has @ism:cuiControlledBy, 
            then it must have ism:complieswith="USA-CUI" or "USA-CUI-ONLY".
        </sch:assert>
    </sch:rule>
</sch:pattern>
