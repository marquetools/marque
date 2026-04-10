<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00363">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00363][Error] HCS-O cannot be used with OC-USGOV.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        When OC-USGOV @ism:disseminationControls is used, HCS-O cannot be in @ism:SCIcontrols.
    </sch:p>
    <sch:rule id="ISM-ID-00363-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV')) and @ism:SCIcontrols]">
        <sch:assert test="not(util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-O')))" flag="error" role="error">
            [ISM-ID-00363][Error] HCS-O cannot be used with OC-USGOV.
        </sch:assert>
    </sch:rule>
</sch:pattern>