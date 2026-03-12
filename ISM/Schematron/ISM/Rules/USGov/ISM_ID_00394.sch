<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00394">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00394][Error] If the ISM_RESOURCE_ELEMENT has the "RAWFISA" dissemination control 
        and no compilation reason, then at least one portion must have the "RAWFISA" dissemination control.
        
        Human Readable: USA documents marked RAWFISA at the resource level must have RAWFISA data.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="codeDesc">For the ISM_RESOURCE_ELEMENT with attribute @ism:disseminationControls 
        containing the name token "RAWFISA" and no @ism:compilationReason, then some portion of 
        the document must have @ism:disseminationControls containing the "RAWFISA" token.
    </sch:p>
    <sch:rule id="ISM-ID-00394-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RAWFISA')) and not(@ism:compilationReason)]">
        <sch:assert test="index-of($partDisseminationControls_tok, 'RAWFISA') &gt; 0" flag="error" role="error">
            [ISM-ID-00394][Error] If the ISM_RESOURCE_ELEMENT has the "RAWFISA" dissemination control 
            and no compilation reason, then at least one portion must have the "RAWFISA" dissemination control.
            
            Human Readable: USA documents marked RAWFISA at the resource level must have RAWFISA data.
        </sch:assert>
    </sch:rule>
</sch:pattern>
