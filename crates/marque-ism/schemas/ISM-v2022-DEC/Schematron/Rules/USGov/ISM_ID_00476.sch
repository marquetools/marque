<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00476">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="ruleText"> [ISM-ID-00476][Error] If @ism:compliesWith="USA-CUI-ONLY" then attributes
        @ism:SCIcontrols, @ism:SARIdentifier, @ism:atomicEnergyMarkings, @ism:FGIsourceOpen and
        @ism:FGIsourceProtected must not be specified. </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="codeDesc"> If the document has @ism:compliesWith="USA-CUI-ONLY", as defined in
        variable ISM_USCUIONLY_RESOURCE, this rule ensures that NONE of the following attributes are
        specified: @ism:SCIcontrols, @ism:SARIdentifier, @ism:atomicEnergyMarkings,
        @ism:FGIsourceOpen and @ism:FGIsourceProtected . </sch:p>
    <sch:rule id="ISM-ID-00476-R1" context="*[$ISM_USCUIONLY_RESOURCE]">
        <sch:assert
            test="not(@ism:SARIdentifier or @ism:SCIcontrols or @ism:atomicEnergyMarkings or @ism:FGIsourceOpen or @ism:FGIsourceProtected)"
            flag="error" role="error"> [ISM-ID-00476][Error] If @ism:compliesWith="USA-CUI-ONLY",
            then attributes @ism:SCIcontrols, @ism:SARIdentifier, @ism:atomicEnergyMarkings,
            @ism:FGIsourceOpen and @ism:FGIsourceProtected must not be specified. </sch:assert>
    </sch:rule>
</sch:pattern>
