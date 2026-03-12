<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00398">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00398][Error] If ISM_USGOV_RESOURCE and @ism:attribute SCIcontrols
        contains a name token that complies with the pattern [KLM-X-Y], where X and Y are any alphanumeric
        strings of any length, then attribute @ism:disseminationControls must contain the name token [OC].
        
        Human Readable: A USA document with any KLM subcompartments must be marked for ORIGINATOR CONTROLLED (ORCON) dissemination.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing a token
        following the pattern [KLM-X-Y], where X and Y are any alphanumeric strings of any length, this rule ensures that attribute
        @ism:disseminationControls is specified with a value containing the token [OC].  
    </sch:p>
    <sch:rule id="ISM-ID-00398-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-[A-Z0-9]*-[A-Z0-9]*$'))]">
      <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))" flag="error" role="error">
          [ISM-ID-00398][Error] If ISM_USGOV_RESOURCE and @ism:attribute SCIcontrols
          contains a name token that complies with the pattern [KLM-X-Y], where X and Y are any alphanumeric
          strings of any length, then attribute @ism:disseminationControls must contain the name token [OC].
          
          Human Readable: A USA document with any KLM subcompartments must be marked for ORIGINATOR CONTROLLED (ORCON) dissemination.
        </sch:assert>
    </sch:rule>
</sch:pattern>