<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00341">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00341][Error] If ISM_USGOV_RESOURCE and @ism:SCIcontrols contains a token matching [SI-G]
        or [SI-G-XXXX], then @ism:disseminationControls cannot contain [OC-USGOV].
        
        Human Readable: OC-USGOV cannot be used if SI-G or an SI-G subs are present. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE and @ism:SCIcontrols contains [SI-G] or [SI-G-XXXX], 
        then @ism:disseminationControls cannot contain [OC-USGOV].
    </sch:p>
    <sch:rule id="ISM-ID-00341-R1" context="*[$ISM_USGOV_RESOURCE and (util:containsAnyTokenMatching(@ism:SCIcontrols, ('^SI-G-[A-Z]{4}$'))) or util:containsAnyOfTheTokens(@ism:SCIcontrols, ('SI-G'))]">
        <sch:assert test="not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV')))" flag="error" role="error">
            [ISM-ID-00341][Error] If ISM_USGOV_RESOURCE and @ism:SCIcontrols contains a token matching [SI-G]
            or [SI-G-XXXX], then @ism:disseminationControls cannot contain [OC-USGOV].
            
            Human Readable: OC-USGOV cannot be used if SI-G or an SI-G subs are present. 
        </sch:assert>
    </sch:rule>
</sch:pattern>