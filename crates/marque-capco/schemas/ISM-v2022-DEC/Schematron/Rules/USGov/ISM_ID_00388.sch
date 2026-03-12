<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00388">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    [ISM-ID-00388][Error] If ISM_USGOV_RESOURCE and @ism:attribute SCIcontrols contains a token matching containing a "-" 
    then it must also contain the token before the "-". This is to ensure all compartments specify the control system 
    and all subcompartments specify the compartment. 
    
    Human Readable: A USA document with a SCI compartment must specify the control system, 
    also a SCI subcompartment must specify the compartment. 
  </sch:p>
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    If ISM_USGOV_RESOURCE and attribute SCIcontrols contains a token matching containing a "-" 
    then it must also contain the token before the "-". This is to ensure all compartments specify the control system 
    and all subcompartments specify the compartment.
  </sch:p>
  <sch:rule id="ISM-ID-00388-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^.*-[A-Z]'))]">
    <sch:let name="allSCI" value="util:tokenize(@ism:SCIcontrols)"/>
    <sch:assert test="every $token in $allSCI satisfies (not(matches($token,'^.*-[A-Z]')) or (util:containsAnyOfTheTokens(@ism:SCIcontrols, string(util:before-last-delimeter($token,'-'))) ))" flag="error" role="error">
      [ISM-ID-00388][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains a token containing a "-" then it must also contain the token before the "-". This is to ensure 
      all compartments specify the control system and all subcompartments specify the compartment. The following token(s) do not meet this criteria (
      <sch:value-of select="for $token in $allSCI return if (not(matches($token,'^.*-[A-Z]')) or (util:containsAnyOfTheTokens(@ism:SCIcontrols, string(util:before-last-delimeter($token,'-'))) ))
        then null else $token"/> )
    </sch:assert>
    </sch:rule>
</sch:pattern>